//! Localized user-facing strings. English is the default; ES/FR/NL/IT/DE are supported.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
    Fr,
    Nl,
    It,
    De,
}

impl Lang {
    /// Map an ISO 639-1 primary language code to a supported language (English otherwise).
    pub fn from_code(code: &str) -> Lang {
        match code.to_ascii_lowercase().as_str() {
            "es" => Lang::Es,
            "fr" => Lang::Fr,
            "nl" => Lang::Nl,
            "it" => Lang::It,
            "de" => Lang::De,
            _ => Lang::En,
        }
    }
}

/// Prompt shown when the input folder was just created / is empty.
pub fn drop_images_prompt(lang: Lang, folder: &str) -> String {
    let body = match lang {
        Lang::En => "Drop your images into the \"{}\" folder on your desktop, then start this app again.",
        Lang::Es => "Coloca tus imágenes en la carpeta \"{}\" del escritorio y vuelve a iniciar esta aplicación.",
        Lang::Fr => "Placez vos images dans le dossier « {} » du bureau, puis relancez cette application.",
        Lang::Nl => "Zet je afbeeldingen in de map \"{}\" op je bureaublad en start deze app opnieuw.",
        Lang::It => "Inserisci le tue immagini nella cartella \"{}\" sul desktop, poi riavvia questa app.",
        Lang::De => "Lege deine Bilder in den Ordner \"{}\" auf dem Desktop und starte diese App erneut.",
    };
    body.replacen("{}", folder, 1)
}

/// Completion alert. `seconds` is already rounded to hundredths.
pub fn batch_finished(lang: Lang, seconds: f64, ok: usize, failed: usize) -> String {
    let s = format!("{seconds:.2}");
    match lang {
        Lang::En => format!("Batch finished in {s}s. {ok} processed, {failed} skipped."),
        Lang::Es => format!("Lote terminado en {s}s. {ok} procesadas, {failed} omitidas."),
        Lang::Fr => format!("Traitement terminé en {s}s. {ok} traitées, {failed} ignorées."),
        Lang::Nl => format!("Batch klaar in {s}s. {ok} verwerkt, {failed} overgeslagen."),
        Lang::It => format!("Lotto completato in {s}s. {ok} elaborate, {failed} saltate."),
        Lang::De => format!("Stapel in {s}s fertig. {ok} verarbeitet, {failed} übersprungen."),
    }
}

/// Short window title for the alerts.
pub fn app_title() -> &'static str {
    "Image Batch"
}
