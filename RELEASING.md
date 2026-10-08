# Préparation de publication

Statut : version 0.1.0 sous GNU GPL version 3 uniquement (`GPL-3.0-only`), texte officiel dans LICENSE et métadonnées Cargo renseignées. Dépôt public : https://github.com/Bertollo-Nicolas/PokerForge. Aucun tag de release ni publication crates.io ; `publish = false` reste activé pour contrôler séparément cette publication.

## Vérifications avant diffusion

1. Vérifier que LICENSE et la déclaration `GPL-3.0-only` accompagnent la diffusion. Conserver les avis de licence des dépendances ; aucun nom de titulaire n'est inventé par les outils.
2. Vérifier la CI Windows/Linux du commit à diffuser sur le dépôt GitHub, puis créer le tag et la release souhaités. Les validations locales ne remplacent pas ces jobs.
3. Vérifier `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --release` et `cargo run --locked --release --example fuzz_smoke -- 20000`.
4. Exécuter la cible cargo-fuzz sous Linux nightly avec sanitizers ; conserver les inputs minimisés qui révèlent une erreur comme tests de régression synthétiques.
5. Relancer le benchmark avec les métadonnées du binaire réellement publié et joindre les résultats, sans annoncer un plafond universel.
6. Contrôler `cargo package --list --allow-dirty` : la liste d'inclusion explicite exclut data, target, captures ETL et corpus de fuzz local, indépendamment des règles Git.
7. Vérifier les fichiers du dépôt avant push : seuls les exemples synthétiques sont destinés à être partagés. Ne pas ajouter data/, target/, fuzz/corpus/ ou fuzz/artifacts/.

## Fuzzing

Sans installation additionnelle, campagne de mutations reproductibles :

```text
cargo run --release --example fuzz_smoke -- 20000
```

La graine fixe permet de reproduire les cas. Cette campagne compare statistiques, faits et offsets entre les deux chemins de lecture ; elle vérifie leur cohérence et l'absence de panique, pas la légalité de chaque main de poker.

Sous Linux avec Rust nightly et cargo-fuzz :

```sh
cargo install cargo-fuzz --locked
mkdir -p fuzz/corpus/parser
cp tests/fixtures/*.txt fuzz/corpus/parser/
cargo +nightly fuzz run parser -- -max_total_time=300 -max_len=65536
```

La cible coverage-guided est séparée dans fuzz/ ; elle n'ajoute aucune dépendance au parseur de production. Sa compilation a été vérifiée localement sur Windows. Une campagne instrumentée sous Ubuntu WSL a terminé le 8 octobre 2026 : 1 513 839 exécutions en 301 secondes, sans incident. La CI prévoit également 30 secondes par exécution, mais aucun run GitHub n'a encore eu lieu.

Pour reproduire les vérifications avec Rust stable/nightly et cargo-fuzz déjà installés : `bash scripts/validate-linux.sh 300`. Le corpus initial est synthétique ; les mutations locales restent ignorées par Git. Voir `benches/LINUX-VALIDATION.md` pour les versions et les limites de cette validation.

## Plateformes et reproductibilité

Rust minimal déclaré : 1.90. CLI Rust portable ; scripts PowerShell de mesure de mémoire et outils WPR/xperf spécifiques à Windows. La CI ne dépend pas du corpus privé. Les tests privés se désactivent si data/ n'existe pas. Les mesures de performance ne constituent pas des seuils de réussite de la CI, afin de ne pas confondre bruit des machines partagées et régression fonctionnelle.
