# Validation Linux — 8 octobre 2026

Exécution locale sous Ubuntu WSL2, x86_64, noyau 6.6.87.2-microsoft-standard-WSL2. Projet sur le volume Windows monté dans WSL. Ce contrôle fonctionnel n'est pas une mesure de débit Linux natif.

- Rust stable 1.99.0 (b940084d7, 2026-09-28) : formatage, Clippy `--locked --all-targets -- -D warnings`, 28 tests release réussis, y compris les tests du corpus privé disponible.
- Rust nightly 1.101.0-nightly (1d81eb4ad, 2026-10-07), LLVM 23.1.3 : mêmes vérifications réussies, formatage du projet fuzz, 100 000 mutations déterministes réussies et CLI streaming sur fixture synthétique (1 main, 3 joueurs, 12 actions, checksum `8ca2d2e4c346206c`).
- cargo-fuzz 0.13.2, libfuzzer-sys 0.4.13, instrumentation AddressSanitizer et couverture : cible `parser`, graine 642689237, `-max_total_time=300 -max_len=65536`, départ avec la seule fixture synthétique split-pot de 1012 octets.
- Résultat libFuzzer : `Done 1513839 runs in 301 second(s)`, code de sortie 0. Aucune panique, divergence des statistiques/faits/offsets entre lecture intégrale et streaming, ni erreur de sanitizer signalée.

Les outils Linux ont été installés dans `target/linux-tools/`, sans modifier le PATH utilisateur ; les builds sont dans `target/linux-validation/`. Les entrées générées restent dans `fuzz/corpus/parser/`, ignoré par Git. Aucune donnée privée n'a servi de graine au fuzzing.

Reproduction avec les outils requis déjà disponibles : `bash scripts/validate-linux.sh 300`. Le corpus local existant est conservé entre campagnes ; supprimer ou déplacer ce corpus volontairement est nécessaire pour repartir uniquement de la fixture. Une graine seule ne garantit pas une séquence identique entre versions et machines.

Cette campagne courte n'est pas une preuve exhaustive de correction du parsing, ni une validation de toutes les règles du poker. La version minimale déclarée Rust 1.90 n'a pas été testée lors de cette campagne. Les jobs GitHub Actions n'ont pas été exécutés ; aucune publication ni modification de licence.
