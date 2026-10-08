# Validation indépendante des HandFacts

Commande : `cargo test --locked --release --test differential -- --nocapture`.

Le parseur dans `support/reference.rs` est réservé aux tests. Il utilise UTF-8, String, Vec, la tokenisation et les conversions numériques standard. Il ne réutilise ni scanner, ni helpers de parsing, ni checksum du code de production. Il partage le modèle public de sortie et sa convention FNV-1a pour l'identifiant composite. Ce hash n'est pas une identité sans collision pour un futur tracker.

Trois contrôles complémentaires :

- Huit traces synthétiques avec tous les champs attendus explicites, en LF, CRLF et sans saut final : heads-up avec sièges vides, limp/check, seconde relance all-in courte, quatrième mise préflop, nom contenant un verbe avec ordre des sièges inversé, agressivité postflop, board complet et split pot, ESCAPE et contribution au pot. Les emplacements joueurs inutilisés restent nuls.
- Dix altérations rejetées par les deux parseurs : résumé absent, total absent, siège dupliqué, acteur inconnu, précision monétaire excessive, fraction vide, action inconnue, carte dupliquée, board résumé différent, progression incohérente.
- Découpage indépendant par lignes puis comparaison de tous les champs du corpus synthétique concaténé avec BOM et des TXT privés récursifs disponibles. Le corpus privé est facultatif et non distribué. Une erreur I/O, UTF-8 ou de parsing fait échouer le test ; aucune main n'est écartée pour obtenir une concordance.

Résultat local du 8 octobre 2026 : 67 127 mains dans 536 fichiers, sans divergence. Les trois tests passent sous Windows Rust 1.90 et Ubuntu WSL Rust stable 1.99. La suite complète Windows compte désormais 31 tests ; formatage et Clippy passent sur les deux plateformes.

Le parseur de référence couvre l'extraction sur les historiques pris en charge et les rejets ciblés ci-dessus. Il est moins strict sur certaines lignes ignorées (posts, shows, collected) et n'est pas un oracle général d'acceptation/rejet pour des octets arbitraires. Les traces synthétiques isolent les règles d'extraction ; les totaux de pot ne prouvent pas leur cohérence comptable. Aucun des deux parseurs n'est un moteur de légalité du poker. `three_bet` désigne la deuxième action raises préflop, même courte et sans réouverture légale des mises. Les tests de streaming/parallélisme existants complètent ces contrôles en comparant les faits du chemin rapide entre modes.
