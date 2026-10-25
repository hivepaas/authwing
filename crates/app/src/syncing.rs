//! Syncing the vault with a folder, a WebDAV server or S3-compatible storage
//! (the place, [`SyncPlace`]), on a thread of its own: every minute, and
//! right after something asks (a code added, changed or deleted, Sync now).
//!
//! Pairing goes through the place too: a new device's request to join (see
//! `authwing_vault::Ask`), and the answers of a device that syncs. While one
//! is open the place is looked at every two seconds.
//!
//! Only the vault syncs: config.toml is each device's own.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use authwing_config::SyncPlace;
use authwing_i18n::tr;
use authwing_remote::Place;
use authwing_vault::{
    Answerer, Ask, AskState, Backend, FolderState, Request, SyncReport, Vault, transact,
};

use crate::vault_slot::VaultSlot;

const EVERY: Duration = Duration::from_secs(60);
/// How often the place is looked at while pairing.
const PAIRING: Duration = Duration::from_secs(2);
/// The same over Git, where each look is a fetch.
const PAIRING_GIT: Duration = Duration::from_secs(5);

pub enum SyncEvent {
    /// A sync starts.
    Started,
    Done(SyncReport),
    Failed(String),
    /// This device's request to join: where it stands.
    Asking(AskState),
    /// Requests from new devices, ready to approve or refuse.
    Requests(Vec<Request>),
    /// A step of pairing failed.
    PairingFailed(String),
    /// Whether a passphrase joins new devices to the vault there.
    PassphraseJoins(bool),
}

/// The vault's own secret of the place (a WebDAV password, an S3 secret
/// key), kept as this device's.
pub const PLACE_SECRET: &str = "sync";

/// The place with its secret from the vault, ready to open.
pub fn resolve(place: &SyncPlace, vault: &Vault) -> authwing_vault::Result<Place> {
    // A token for Git is not needed: git signs in as it does for the user.
    let token = || -> authwing_vault::Result<String> {
        Ok(vault
            .device_secret(PLACE_SECRET)?
            .map(|s| s.to_string())
            .unwrap_or_default())
    };
    let secret = || -> authwing_vault::Result<String> {
        match vault.device_secret(PLACE_SECRET)? {
            Some(s) => Ok(s.to_string()),
            None => Err(authwing_vault::Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                tr!("its password is not saved on this device: connect again in Settings › Sync"),
            ))),
        }
    };
    Ok(match place {
        SyncPlace::Folder(f) => Place::Folder(f.clone()),
        SyncPlace::WebDav(w) => Place::WebDav {
            url: w.url.clone(),
            user: w.user.clone(),
            password: secret()?,
        },
        SyncPlace::S3(s3) => Place::S3 {
            endpoint: s3.endpoint.clone(),
            region: s3.region.clone(),
            bucket: s3.bucket.clone(),
            prefix: s3.prefix.clone(),
            access_key: s3.access_key.clone(),
            secret_key: secret()?,
        },
        SyncPlace::Git(g) => Place::Git {
            url: g.url.clone(),
            branch: g.branch.clone(),
            user: g.user.clone(),
            token: token()?,
            // In every commit: the random id, not the computer's name.
            device: vault.device_id().to_string(),
        },
    })
}

/// How `place` is named to the user.
pub fn describe(place: &SyncPlace) -> String {
    let unsealed = match place {
        SyncPlace::Folder(f) => Place::Folder(f.clone()),
        SyncPlace::WebDav(w) => Place::WebDav {
            url: w.url.clone(),
            user: w.user.clone(),
            password: String::new(),
        },
        SyncPlace::S3(s3) => Place::S3 {
            endpoint: s3.endpoint.clone(),
            region: s3.region.clone(),
            bucket: s3.bucket.clone(),
            prefix: s3.prefix.clone(),
            access_key: String::new(),
            secret_key: String::new(),
        },
        SyncPlace::Git(g) => Place::Git {
            url: g.url.clone(),
            branch: g.branch.clone(),
            user: g.user.clone(),
            token: String::new(),
            device: String::new(),
        },
    };
    unsealed.describe()
}

/// Opens the place's backend with its secret.
pub fn open(place: &SyncPlace, vault: &Vault) -> authwing_vault::Result<Arc<dyn Backend>> {
    Ok(resolve(place, vault)?.open()?)
}

/// The backend of the place now, opened again only when the place (or its
/// secret) changed: a remote one keeps what it read.
#[derive(Default)]
struct Opened(Option<(Place, Arc<dyn Backend>)>);

impl Opened {
    fn get(
        &mut self,
        place: &SyncPlace,
        vault: &Vault,
    ) -> authwing_vault::Result<Arc<dyn Backend>> {
        let resolved = resolve(place, vault)?;
        if let Some((p, b)) = &self.0
            && *p == resolved
        {
            return Ok(b.clone());
        }
        let backend = resolved.open()?;
        self.0 = Some((resolved, backend.clone()));
        Ok(backend)
    }
}

enum Cmd {
    Sync,
    /// A sync, then a word back once it is over (or nothing could sync).
    SyncThen(mpsc::Sender<()>),
    /// Ask to join the vault there, as this device.
    Ask(SyncPlace),
    CancelAsk,
    Approve(String),
    Refuse(String),
}

#[derive(Clone)]
pub struct SyncWorker {
    wake: mpsc::Sender<Cmd>,
    place: Arc<Mutex<Option<SyncPlace>>>,
}

/// This device's name, shown to the other when pairing.
pub fn device_name() -> String {
    authwing_vault::host_name().unwrap_or_else(|| "another device".into())
}

impl SyncWorker {
    pub fn start(vault: VaultSlot, events: async_channel::Sender<SyncEvent>) -> SyncWorker {
        let (wake, woken) = mpsc::channel::<Cmd>();
        let place: Arc<Mutex<Option<SyncPlace>>> = Arc::default();
        let watched = place.clone();
        let spawned = std::thread::Builder::new()
            .name("authwing-sync".into())
            .spawn(move || {
                let mut answerer = Answerer::new(&device_name());
                let mut asking: Option<(Ask, SyncPlace, Arc<dyn Backend>)> = None;
                let mut opened = Opened::default();
                // What the app was last told of the passphrase that joins.
                let mut joins: Option<bool> = None;
                let mut offered: Vec<Request> = vec![];
                // What the app was last told of this device's request.
                let mut told: Option<AskState> = None;
                // Due at once: the first sync runs at start.
                let mut last_sync: Option<Instant> = None;
                // Those waiting for a sync to be over: told when the round
                // that took their wake is done.
                let mut acks: Vec<mpsc::Sender<()>> = vec![];
                loop {
                    for a in acks.drain(..) {
                        let _ = a.send(());
                    }
                    let pairing = asking.is_some() || answerer.busy();
                    let over_git = matches!(
                        watched.lock().ok().and_then(|f| f.clone()),
                        Some(SyncPlace::Git(_))
                    ) || matches!(&asking, Some((_, SyncPlace::Git(_), _)));
                    let wait = match (pairing, over_git) {
                        (false, _) => EVERY,
                        (true, false) => PAIRING,
                        (true, true) => PAIRING_GIT,
                    };
                    let mut sync = match woken.recv_timeout(wait) {
                        Ok(Cmd::SyncThen(a)) => {
                            acks.push(a);
                            true
                        }
                        Ok(cmd) => Self::run(
                            cmd,
                            &vault,
                            &mut asking,
                            &mut answerer,
                            &mut opened,
                            &watched,
                            &events,
                        ),
                        Err(RecvTimeoutError::Timeout) => false,
                        Err(RecvTimeoutError::Disconnected) => return,
                    };
                    // Several wakes in a row are one sync.
                    while let Ok(cmd) = woken.try_recv() {
                        if let Cmd::SyncThen(a) = cmd {
                            acks.push(a);
                            sync = true;
                            continue;
                        }
                        sync |= Self::run(
                            cmd,
                            &vault,
                            &mut asking,
                            &mut answerer,
                            &mut opened,
                            &watched,
                            &events,
                        );
                    }
                    let Some(vault) = vault.get() else {
                        continue;
                    };
                    if asking.is_none() {
                        told = None;
                    }
                    if let Some((ask, at, backend)) = &mut asking {
                        let state = transact(backend.as_ref(), |b| ask.step(&vault, b));
                        // Over once in, refused or expired, or when the keys sent
                        // are no good; a folder that cannot be read is tried again.
                        let done = matches!(
                            state,
                            Ok(AskState::Joined | AskState::Refused | AskState::Expired)
                                | Err(authwing_vault::Error::Pairing(_))
                        );
                        if matches!(state, Ok(AskState::Joined)) {
                            if let Ok(mut f) = watched.lock() {
                                *f = Some(at.clone());
                            }
                            sync = true;
                        }
                        // Told again only when it changed.
                        let event = match state {
                            Ok(s) if told.as_ref() == Some(&s) => None,
                            Ok(s) => {
                                told = Some(s.clone());
                                Some(SyncEvent::Asking(s))
                            }
                            Err(e) => Some(SyncEvent::PairingFailed(e.to_string())),
                        };
                        if let Some(event) = event
                            && events.send_blocking(event).is_err()
                        {
                            return;
                        }
                        if done {
                            asking = None;
                        }
                    }
                    let Some(place) = watched.lock().ok().and_then(|f| f.clone()) else {
                        joins = None;
                        continue;
                    };
                    let store = match opened.get(&place, &vault) {
                        Ok(b) => b,
                        Err(e) => {
                            if events
                                .send_blocking(SyncEvent::Failed(e.to_string()))
                                .is_err()
                            {
                                return;
                            }
                            continue;
                        }
                    };
                    let store = store.as_ref();
                    match transact(store, |b| answerer.poll(&vault, b)) {
                        Ok(ready) if ready != offered => {
                            offered = ready.clone();
                            if events.send_blocking(SyncEvent::Requests(ready)).is_err() {
                                return;
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            let _ = events.send_blocking(SyncEvent::PairingFailed(e.to_string()));
                        }
                    }
                    let due = last_sync.is_none_or(|t| t.elapsed() >= EVERY);
                    if !sync && !due {
                        continue;
                    }
                    last_sync = Some(Instant::now());
                    if events.send_blocking(SyncEvent::Started).is_err() {
                        return;
                    }
                    let event = sync_once(&vault, store);
                    if events.send_blocking(event).is_err() {
                        return;
                    }
                    if let Ok(now) = vault.passphrase_joins(store)
                        && joins != Some(now)
                    {
                        joins = Some(now);
                        if events
                            .send_blocking(SyncEvent::PassphraseJoins(now))
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("cannot start syncing: {e}");
        }
        SyncWorker { wake, place }
    }

    /// Syncs with `place` from now on (None: stops), starting right away.
    pub fn set_place(&self, place: Option<SyncPlace>) {
        let changed = self
            .place
            .lock()
            .map(|mut f| {
                let changed = *f != place;
                *f = place;
                changed
            })
            .unwrap_or(false);
        if changed {
            self.now();
        }
    }

    pub fn place(&self) -> Option<SyncPlace> {
        self.place.lock().ok().and_then(|f| f.clone())
    }

    pub fn now(&self) {
        let _ = self.wake.send(Cmd::Sync);
    }

    /// Asks for a sync and waits for it to be over, `most` at the longest
    /// (quitting: the last edits go out first). False when it took longer.
    pub fn sync_and_wait(&self, most: Duration) -> bool {
        let (tx, rx) = mpsc::channel();
        if self.wake.send(Cmd::SyncThen(tx)).is_err() {
            return true;
        }
        rx.recv_timeout(most).is_ok()
    }

    /// Asks to join the vault at `place` from a device that syncs it; how it
    /// goes comes as [`SyncEvent::Asking`].
    pub fn ask_to_join(&self, place: SyncPlace) {
        let _ = self.wake.send(Cmd::Ask(place));
    }

    pub fn cancel_ask(&self) {
        let _ = self.wake.send(Cmd::CancelAsk);
    }

    /// Lets the device of request `id` in.
    pub fn approve(&self, id: String) {
        let _ = self.wake.send(Cmd::Approve(id));
    }

    pub fn refuse(&self, id: String) {
        let _ = self.wake.send(Cmd::Refuse(id));
    }

    /// Carries out a command on the sync thread; whether a sync should follow.
    fn run(
        cmd: Cmd,
        vault: &VaultSlot,
        asking: &mut Option<(Ask, SyncPlace, Arc<dyn Backend>)>,
        answerer: &mut Answerer,
        opened: &mut Opened,
        watched: &Mutex<Option<SyncPlace>>,
        events: &async_channel::Sender<SyncEvent>,
    ) -> bool {
        let fail = |e: authwing_vault::Error| {
            let _ = events.send_blocking(SyncEvent::PairingFailed(e.to_string()));
            false
        };
        // The place synced now, opened.
        let mut current = |vault: &Vault| {
            let place = watched.lock().ok().and_then(|f| f.clone())?;
            opened.get(&place, vault).ok()
        };
        match cmd {
            // (Taken before run, with its word back.)
            Cmd::Sync | Cmd::SyncThen(_) => true,
            Cmd::Ask(place) => {
                if let Some((old, _, at)) = asking.take() {
                    let _ = transact(at.as_ref(), |b| old.cancel(b));
                }
                let Some(v) = vault.get() else {
                    return false;
                };
                let backend = match open(&place, &v) {
                    Ok(b) => b,
                    Err(e) => return fail(e),
                };
                match transact(backend.as_ref(), |b| Ask::new(b, &device_name())) {
                    Ok(ask) => *asking = Some((ask, place, backend)),
                    Err(e) => return fail(e),
                }
                false
            }
            Cmd::CancelAsk => {
                if let Some((ask, _, at)) = asking.take() {
                    let _ = transact(at.as_ref(), |b| ask.cancel(b));
                }
                false
            }
            Cmd::Approve(id) => {
                let Some(v) = vault.get() else {
                    return false;
                };
                let Some(store) = current(&v) else {
                    return false;
                };
                match transact(store.as_ref(), |b| answerer.approve(&v, b, &id)) {
                    Ok(()) => false,
                    Err(e) => fail(e),
                }
            }
            Cmd::Refuse(id) => {
                let Some(v) = vault.get() else {
                    return false;
                };
                let Some(store) = current(&v) else {
                    return false;
                };
                match transact(store.as_ref(), |b| answerer.refuse(b, &id)) {
                    Ok(()) => false,
                    Err(e) => fail(e),
                }
            }
        }
    }
}

/// One round: the vault started there if the place is empty, then synced.
fn sync_once(vault: &Vault, store: &dyn Backend) -> SyncEvent {
    // A folder set by hand in config.toml starts empty: begin
    // there (the recovery key can be shown from the palette).
    let started = transact(store, |b| match vault.folder_state(b)? {
        FolderState::Empty => vault.start_sync(b).map(|_| ()),
        _ => Ok(()),
    });
    match started.and_then(|()| vault.sync(store)) {
        Ok(r) => SyncEvent::Done(r),
        Err(e) => SyncEvent::Failed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use authwing_vault::{Folder, MemoryKeys};
    use std::collections::BTreeMap;

    struct Device {
        vault: Vault,
        _keys: MemoryKeys,
    }

    impl Device {
        fn new(root: &std::path::Path, name: &str) -> Device {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).expect("dir");
            let keys = MemoryKeys::default();
            let vault = Vault::open(&dir.join("vault.db"), &keys).expect("vault");
            Device { vault, _keys: keys }
        }

        /// One round, as the worker does it.
        fn sync(&self, folder: &Folder) {
            match sync_once(&self.vault, folder) {
                SyncEvent::Done(_) => {}
                SyncEvent::Failed(e) => panic!("sync: {e}"),
                _ => panic!("not a sync"),
            }
        }
    }

    #[test]
    fn codes_travel_and_a_delete_follows() {
        let root = std::env::temp_dir().join(format!("authwing-codes-sync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let folder = Folder::new(&root.join("sync"));
        let a = Device::new(&root, "a");
        let code = BTreeMap::from([
            ("issuer".to_string(), "GitHub".to_string()),
            ("account".into(), "me@example.com".into()),
            ("secret".into(), "JBSWY3DPEHPK3PXP".into()),
        ]);
        // Ids are hex, as synced files' names are.
        let id = authwing_vault::new_id().expect("id");
        a.vault.update(&id, "code", "GitHub", &code).expect("add");
        a.sync(&folder);
        let rk = a.vault.recovery_key().expect("rk").expect("started");
        let b = Device::new(&root, "b");
        b.vault.join(&folder, &rk.to_string()).expect("join");
        b.sync(&folder);
        assert_eq!(
            b.vault.entries("code").expect("codes"),
            vec![(id.clone(), code)]
        );
        b.vault.delete(&id).expect("delete");
        b.sync(&folder);
        a.sync(&folder);
        assert!(a.vault.entries("code").expect("codes").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
