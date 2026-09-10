//! Localized user-facing strings. English is the default; ES/FR/NL/IT/DE are supported.
//!
//! Every line here is printed inside the layout `app::theme` draws, so keep each one under
//! `theme::WIDTH` characters — a longer line wraps and pushes the rule below it out of place.

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

/// First standing line of the banner. The folder path is printed separately, below it.
pub fn drop_prompt(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Put your images into the folder below, then double-click csp.exe again.",
        Lang::Es => "Coloca tus imágenes en la carpeta de abajo y vuelve a hacer doble clic en csp.exe.",
        Lang::Fr => "Placez vos images dans le dossier ci-dessous, puis double-cliquez à nouveau sur csp.exe.",
        Lang::Nl => "Zet je afbeeldingen in de map hieronder en dubbelklik opnieuw op csp.exe.",
        Lang::It => "Inserisci le immagini nella cartella qui sotto, poi fai di nuovo doppio clic su csp.exe.",
        Lang::De => "Lege deine Bilder in den Ordner unten und doppelklicke csp.exe erneut.",
    }
}

/// Second standing line of the banner.
pub fn close_line(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Press any key to close this window.",
        Lang::Es => "Presiona cualquier tecla para cerrar esta ventana.",
        Lang::Fr => "Appuyez sur une touche pour fermer cette fenêtre.",
        Lang::Nl => "Druk op een toets om dit venster te sluiten.",
        Lang::It => "Premi un tasto per chiudere questa finestra.",
        Lang::De => "Drücke eine beliebige Taste, um dieses Fenster zu schließen.",
    }
}

/// Callout shown while the input folder is waiting to be filled.
pub fn ready_hint(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "When your images are ready for processing, double-click csp.exe again to start processing.",
        Lang::Es => "Cuando tus imágenes estén listas, vuelve a hacer doble clic en csp.exe para procesarlas.",
        Lang::Fr => "Lorsque vos images sont prêtes, double-cliquez à nouveau sur csp.exe pour lancer le traitement.",
        Lang::Nl => "Als je afbeeldingen klaarstaan, dubbelklik opnieuw op csp.exe om te verwerken.",
        Lang::It => "Quando le tue immagini sono pronte, fai di nuovo doppio clic su csp.exe per elaborarle.",
        Lang::De => "Wenn deine Bilder bereit sind, doppelklicke csp.exe erneut, um sie zu verarbeiten.",
    }
}

/// Callout shown under the progress bar while the batch runs.
pub fn interrupt_hint(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Push CTRL+C or close this window to interrupt the processing",
        Lang::Es => "Pulsa CTRL+C o cierra esta ventana para interrumpir el procesamiento",
        Lang::Fr => "Appuyez sur CTRL+C ou fermez cette fenêtre pour interrompre le traitement",
        Lang::Nl => "Druk op CTRL+C of sluit dit venster om het verwerken te onderbreken",
        Lang::It => "Premi CTRL+C o chiudi questa finestra per interrompere l'elaborazione",
        Lang::De => "Drücke STRG+C oder schließe dieses Fenster, um die Verarbeitung abzubrechen",
    }
}

/// Printed just before processing begins.
pub fn processing(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Processing your images...",
        Lang::Es => "Procesando tus imágenes...",
        Lang::Fr => "Traitement de vos images...",
        Lang::Nl => "Je afbeeldingen worden verwerkt...",
        Lang::It => "Elaborazione delle tue immagini...",
        Lang::De => "Deine Bilder werden verarbeitet...",
    }
}

/// Completion summary. `seconds` is already rounded to hundredths.
pub fn finished(lang: Lang, seconds: f64, ok: usize, failed: usize) -> String {
    let s = format!("{seconds:.2}");
    match lang {
        Lang::En => format!("Batch finished in {s}s. {ok} processed, {failed} skipped. Your results are in the folder below."),
        Lang::Es => format!("Lote terminado en {s}s. {ok} procesadas, {failed} omitidas. Tus resultados están en la carpeta de abajo."),
        Lang::Fr => format!("Traitement terminé en {s}s. {ok} traitées, {failed} ignorées. Vos résultats sont dans le dossier ci-dessous."),
        Lang::Nl => format!("Batch klaar in {s}s. {ok} verwerkt, {failed} overgeslagen. Je resultaten staan in de map hieronder."),
        Lang::It => format!("Lotto completato in {s}s. {ok} elaborate, {failed} saltate. I risultati sono nella cartella qui sotto."),
        Lang::De => format!("Stapel in {s}s fertig. {ok} verarbeitet, {failed} übersprungen. Deine Ergebnisse sind im Ordner unten."),
    }
}
