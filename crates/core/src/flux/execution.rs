//! Lancement d'un outil externe : sans shell, sans fenêtre console, avec délai.

use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::ErreurFlux;

/// Intervalle de surveillance du processus.
const PAS: Duration = Duration::from_millis(20);

/// Lance `exe args…`, attend la fin (au plus `delai`) et renvoie la sortie
/// standard. Un code de retour non nul est une erreur portant la sortie
/// d'erreur. Le processus est tué si le délai est dépassé.
pub(super) fn executer(
    exe: &Path,
    args: &[OsString],
    delai: Duration,
) -> Result<String, ErreurFlux> {
    let outil = exe
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut commande = Command::new(exe);
    commande
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    sans_fenetre(&mut commande);
    let mut enfant = commande.spawn().map_err(|source| ErreurFlux::Lancement {
        outil: outil.clone(),
        source,
    })?;

    // Lecture dans des fils séparés pour ne pas bloquer sur un tube plein.
    let lire = |flux: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut octets = Vec::new();
            if let Some(mut f) = flux {
                let _ = f.read_to_end(&mut octets);
            }
            octets
        })
    };
    let fil_sortie = lire(
        enfant
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let fil_erreur = lire(
        enfant
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );

    let debut = Instant::now();
    let statut = loop {
        match enfant.try_wait() {
            Ok(Some(statut)) => break statut,
            Ok(None) if debut.elapsed() >= delai => {
                let _ = enfant.kill();
                let _ = enfant.wait();
                return Err(ErreurFlux::DelaiDepasse {
                    outil,
                    secondes: delai.as_secs(),
                });
            }
            Ok(None) => std::thread::sleep(PAS),
            Err(source) => {
                let _ = enfant.kill();
                return Err(ErreurFlux::Lancement { outil, source });
            }
        }
    };
    let sortie = fil_sortie.join().unwrap_or_default();
    let erreur = fil_erreur.join().unwrap_or_default();
    if !statut.success() {
        let mut message = String::from_utf8_lossy(&erreur).trim().to_string();
        if message.is_empty() {
            message = String::from_utf8_lossy(&sortie).trim().to_string();
        }
        return Err(ErreurFlux::Echec {
            outil,
            code: statut.code(),
            message,
        });
    }
    Ok(String::from_utf8_lossy(&sortie).into_owned())
}

#[cfg(windows)]
fn sans_fenetre(commande: &mut Command) {
    use std::os::windows::process::CommandExt;
    /// Empêche l'ouverture d'une console pour le processus enfant.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    commande.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn sans_fenetre(_commande: &mut Command) {}
