//! The language Whisper is told to hear.
//!
//! Told nothing, Whisper guesses the language from the audio, and on a
//! phrase of a second or two it guessed wrong a third of the time and took
//! twice as long. So it is told: the system's language by default, or the
//! one the user chose.

/// The languages Whisper's multilingual models hear, by code, each named in
/// English, in Whisper's order. Cantonese is left out: only the large-v3
/// models know it, and the others take its code for another token.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("zh", "Chinese"),
    ("de", "German"),
    ("es", "Spanish"),
    ("ru", "Russian"),
    ("ko", "Korean"),
    ("fr", "French"),
    ("ja", "Japanese"),
    ("pt", "Portuguese"),
    ("tr", "Turkish"),
    ("pl", "Polish"),
    ("ca", "Catalan"),
    ("nl", "Dutch"),
    ("ar", "Arabic"),
    ("sv", "Swedish"),
    ("it", "Italian"),
    ("id", "Indonesian"),
    ("hi", "Hindi"),
    ("fi", "Finnish"),
    ("vi", "Vietnamese"),
    ("he", "Hebrew"),
    ("uk", "Ukrainian"),
    ("el", "Greek"),
    ("ms", "Malay"),
    ("cs", "Czech"),
    ("ro", "Romanian"),
    ("da", "Danish"),
    ("hu", "Hungarian"),
    ("ta", "Tamil"),
    ("no", "Norwegian"),
    ("th", "Thai"),
    ("ur", "Urdu"),
    ("hr", "Croatian"),
    ("bg", "Bulgarian"),
    ("lt", "Lithuanian"),
    ("la", "Latin"),
    ("mi", "Maori"),
    ("ml", "Malayalam"),
    ("cy", "Welsh"),
    ("sk", "Slovak"),
    ("te", "Telugu"),
    ("fa", "Persian"),
    ("lv", "Latvian"),
    ("bn", "Bengali"),
    ("sr", "Serbian"),
    ("az", "Azerbaijani"),
    ("sl", "Slovenian"),
    ("kn", "Kannada"),
    ("et", "Estonian"),
    ("mk", "Macedonian"),
    ("br", "Breton"),
    ("eu", "Basque"),
    ("is", "Icelandic"),
    ("hy", "Armenian"),
    ("ne", "Nepali"),
    ("mn", "Mongolian"),
    ("bs", "Bosnian"),
    ("kk", "Kazakh"),
    ("sq", "Albanian"),
    ("sw", "Swahili"),
    ("gl", "Galician"),
    ("mr", "Marathi"),
    ("pa", "Punjabi"),
    ("si", "Sinhala"),
    ("km", "Khmer"),
    ("sn", "Shona"),
    ("yo", "Yoruba"),
    ("so", "Somali"),
    ("af", "Afrikaans"),
    ("oc", "Occitan"),
    ("ka", "Georgian"),
    ("be", "Belarusian"),
    ("tg", "Tajik"),
    ("sd", "Sindhi"),
    ("gu", "Gujarati"),
    ("am", "Amharic"),
    ("yi", "Yiddish"),
    ("lo", "Lao"),
    ("uz", "Uzbek"),
    ("fo", "Faroese"),
    ("ht", "Haitian Creole"),
    ("ps", "Pashto"),
    ("tk", "Turkmen"),
    ("nn", "Nynorsk"),
    ("mt", "Maltese"),
    ("sa", "Sanskrit"),
    ("lb", "Luxembourgish"),
    ("my", "Burmese"),
    ("bo", "Tibetan"),
    ("tl", "Tagalog"),
    ("mg", "Malagasy"),
    ("as", "Assamese"),
    ("tt", "Tatar"),
    ("haw", "Hawaiian"),
    ("ln", "Lingala"),
    ("ha", "Hausa"),
    ("ba", "Bashkir"),
    ("jw", "Javanese"),
    ("su", "Sundanese"),
];

/// The Whisper language for a locale such as "vi-VN", "pt_BR.UTF-8" or
/// "zh-Hant-TW", if Whisper hears it.
pub fn language_for_locale(locale: &str) -> Option<&'static str> {
    let code = locale
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    // Older and macrolanguage codes for languages Whisper knows by another.
    let code = match code.as_str() {
        "nb" => "no",
        "iw" => "he",
        "in" => "id",
        "ji" => "yi",
        "jv" => "jw",
        "fil" => "tl",
        "yue" | "cmn" => "zh",
        other => other,
    };
    LANGUAGES
        .iter()
        .find(|(known, _)| *known == code)
        .map(|(known, _)| *known)
}

/// The Whisper language for the system's locale, or English when Whisper
/// does not hear it.
pub fn system_language() -> &'static str {
    sys_locale::get_locale()
        .and_then(|locale| language_for_locale(&locale))
        .unwrap_or("en")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_name_their_language() {
        for (locale, language) in [
            ("vi-VN", Some("vi")),
            ("pt_BR.UTF-8", Some("pt")),
            ("zh-Hant-TW", Some("zh")),
            ("EN-us", Some("en")),
            ("de", Some("de")),
            ("sr@latin", Some("sr")),
            ("nb-NO", Some("no")),
            ("fil-PH", Some("tl")),
            ("yue-Hant-HK", Some("zh")),
            ("C", None),
            ("", None),
            ("tlh", None),
        ] {
            assert_eq!(language_for_locale(locale), language, "{locale}");
        }
    }

    #[test]
    fn languages_are_whispers() {
        assert_eq!(LANGUAGES.len(), 99);
        assert_eq!(LANGUAGES[0], ("en", "English"));
        assert_eq!(LANGUAGES[19], ("vi", "Vietnamese"));
        assert!(LANGUAGES.iter().all(|(code, _)| *code != "yue"));
    }

    #[test]
    fn the_system_has_a_language() {
        let language = system_language();
        assert!(LANGUAGES.iter().any(|(code, _)| *code == language));
    }
}
