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

/// Shown when the input folder was just created or is empty. The folder path is printed separately.
pub fn drop_prompt(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Your input folder is ready. Drop your images into the folder below, then double-click csp.exe again.",
        Lang::Es => "Tu carpeta de entrada está lista. Coloca tus imágenes en la carpeta de abajo y vuelve a hacer doble clic en csp.exe.",
        Lang::Fr => "Votre dossier d'entrée est prêt. Placez vos images dans le dossier ci-dessous, puis double-cliquez à nouveau sur csp.exe.",
        Lang::Nl => "Je invoermap is klaar. Zet je afbeeldingen in de map hieronder en dubbelklik opnieuw op csp.exe.",
        Lang::It => "La tua cartella di input è pronta. Inserisci le immagini nella cartella qui sotto, poi fai di nuovo doppio clic su csp.exe.",
        Lang::De => "Dein Eingabeordner ist bereit. Lege deine Bilder in den Ordner unten und doppelklicke csp.exe erneut.",
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

/// The closing line. Contains a `{}` placeholder where the input-folder path is inserted.
pub fn close_line(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Press any key to close this window.\nThen double-click csp.exe when your images are ready inside {} for processing.",
        Lang::Es => "Presiona cualquier tecla para cerrar esta ventana.\nHaz doble clic en csp.exe cuando tus imágenes estén listas dentro de {} para procesarlas.",
        Lang::Fr => "Appuyez sur une touche pour fermer cette fenêtre.\nPuis double-cliquez sur csp.exe lorsque vos images sont prêtes dans {} pour le traitement.",
        Lang::Nl => "Druk op een toets om dit venster te sluiten.\nDubbelklik op csp.exe wanneer je afbeeldingen klaarstaan in {} om te verwerken.",
        Lang::It => "Premi un tasto per chiudere questa finestra.\nFai doppio clic su csp.exe quando le tue immagini sono pronte in {} per l'elaborazione.",
        Lang::De => "Drücke eine beliebige Taste, um dieses Fenster zu schließen.\nDoppelklicke csp.exe, wenn deine Bilder in {} zur Verarbeitung bereitliegen.",
    }
}
