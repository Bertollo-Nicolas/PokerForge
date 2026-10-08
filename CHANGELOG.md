# Changements

## 0.1.0 — non publié

- Parsing Winamax CashGame/ESCAPE Holdem no limit EUR depuis des octets, faits compacts et checksum déterministe.
- Lecture intégrale, traitement parallèle par fichiers ou blocs indépendants, et streaming optionnel.
- Streaming limité par défaut à 8 MiB par main/préambule, configurable via --max-hand-bytes. Dépassement fatal explicite, sans troncature silencieuse.
- Tests synthétiques de faits, comparaison des modes, mutations déterministes et cible libFuzzer séparée.
- Parseur de référence indépendant réservé aux tests, faits attendus explicites et comparaison champ par champ du corpus disponible.
- Workflow CI Windows/Linux, mesures reproductibles et résultats locaux documentés.
- Licence GNU GPL version 3 uniquement (`GPL-3.0-only`), texte intégral inclus.
- Données privées exclues explicitement du paquet Cargo ; publication Cargo désactivée.
