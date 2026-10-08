# Mesures initiales — 7 octobre 2026

Windows x86_64, rustc 1.90.0 / cargo 1.90.0, profil release, thin LTO, 1 codegen-unit. CPU déclaré par Windows : Intel64 Family 6 Model 154 Stepping 3, 20 processeurs logiques. Aucune option target-cpu=native. Chiffres observés sur une exécution de chaque commande, sans intervalle de confiance.

## Avant tout ajout du parallélisme

Fichier choisi : 226 487 octets, 269 mains, 1 316 sièges cumulés, 2 275 actions, 0 erreur. Comptages mains/actions également recoupés avec un script PowerShell indépendant du parseur.

| Mode | Répétitions | Secondes | MB/s | Mains/s | Actions/s | Checksum |
|---|---:|---:|---:|---:|---:|---|
| Single, mémoire | 1000 | 0.626017 | 361.79 | 429701 | 3634085 | 3065afa3745e78d0 |
| Single, lecture + parsing | 1000 | 0.839805 | 269.69 | 320312 | 2708961 | 3065afa3745e78d0 |

Le corpus complet a aussi été mesuré en single-thread avant le parallélisme : 362.39 MB/s en mémoire, 10 répétitions, 0 erreur.

## Après ajout des workers par fichier

Corpus : 268 fichiers, 17 658 406 octets, 21 792 mains, 103 666 sièges cumulés, 158 263 actions par passage. Zéro erreur dans tous les runs ci-dessous.

| Mode | Répétitions | Secondes | MB/s | Mains/s | Actions/s | Checksum |
|---|---:|---:|---:|---:|---:|---|
| Single, mémoire | 100 | 4.245954 | 415.89 | 513241 | 3727383 | ffec05e0e2fedc18 |
| 4 workers, mémoire | 100 | 1.337246 | 1320.51 | 1629618 | 11834999 | ffec05e0e2fedc18 |
| Single, lecture + parsing | 30 | 2.138983 | 247.67 | 305641 | 2219695 | 19939b5d10e60ed4 |
| 4 workers, lecture + parsing | 30 | 0.848039 | 624.68 | 770908 | 5598672 | 19939b5d10e60ed4 |

Les compteurs complets et checksums concordent entre single et multi pour un même nombre de répétitions. Accélération observée en mémoire : 3.18×. Lecture répétée : cache Windows chaud, pas mesure du disque à froid. Répétitions groupées par fichier : forte localité de cache. Le petit fichier de référence ne peut pas exploiter plusieurs workers.

Commandes reproductibles dans le README. Exécuter plusieurs runs sur une machine au repos avant de conclure à une amélioration ; ces premières valeurs sont une référence exploratoire, pas un résultat statistique ni un comparatif avec Hand2Note/PT4/HM3.

## Corpus élargi — même date

570 fichiers, 56 549 655 octets. Après prise en charge explicite de l'en-tête ESCAPE et du supplément `Escape to Pot` : 67 127 mains acceptées, 349 028 sièges, 525 154 actions, 275 rejets. Audit complet : les rejets sont exclusivement `unsupported format` dans 17 fichiers de tournois (258 mains) et leurs 17 résumés. Aucun rejet dans les fichiers CashGame/ESCAPE du corpus. Checksum par passage : `6e0fc167041cc1b0`.

Cinq exécutions par configuration, dix répétitions par fichier, mode mémoire :

| Workers | MB/s médian | Min–max MB/s |
|---|---:|---:|
| 1 | 344.07 | 320.84–365.76 |
| 4 | 847.03 | 810.17–976.02 |

Compteurs et checksum identiques sur les dix exécutions (`4c9d8e06291f90e0` pour 10 répétitions). Données brutes : `expanded-runs.json`. Commandes : `pokerforge data --memory --repeat 10 --threads 1` puis `--threads 4`, chacune lancée cinq fois. Le code de sortie 1 est attendu ici et a été accompagné d'une vérification de 2 750 rejets par run.

Ce corpus est mixte : le débit inclut les octets des tournois rejetés. Ces chiffres ne sont pas une comparaison pure de performance avec le premier corpus. Accélération médiane observée : 2.46×. Aucun profilage CPU effectué à ce stade.

## Mesure ciblée des phases

Commande : `cargo run --release --example phases -- data`. Ce programme est un benchmark de phases, pas un profil CPU par échantillonnage. Il charge les fichiers avant le chrono et exclut explicitement les fichiers contenant au moins un rejet : 536 fichiers acceptés, 56 318 967 octets, 67 127 mains ; 34 fichiers de tournois/résumés exclus. Aucune donnée source modifiée.

Cinq runs, vingt passages complets sur le corpus par run. Ordre des trois phases tournant entre les runs. Contrairement au benchmark CLI précédent, les répétitions sont ici par corpus entier, pas groupées par fichier.

| Phase | Temps médian (s) | MB/s médian |
|---|---:|---:|
| Détection des débuts de mains seule | 0.727687 | 1547.89 |
| Parsing avec frontières précalculées, sans checksum | 2.620835 | 429.78 |
| Pipeline complet, checksum compris | 3.496345 | 322.16 |

Données brutes : `phases.txt`. La détection isolée coûte environ 20.8 % du temps du pipeline complet. Ces mesures ne sont pas additives : frontières précalculées, checksum absent et localité des accès changent les conditions. On ne peut pas attribuer exactement la différence au checksum ou garantir 21 % de gain en supprimant un parcours.

Conclusion exploratoire : mesurer ensuite une détection de lignes/frontières plus efficace ou un parcours fusionné. Aucun changement du parseur de production réalisé pendant cette mesure. Aucun classement des fonctions par temps CPU disponible ; une trace par échantillonnage reste nécessaire pour cela.

## Optimisation des fins de ligne avec memchr

Corpus nettoyé : 536 fichiers, 56 318 967 octets, 67 127 mains. Le binaire précédent a été conservé localement dans `target/release/pokerforge-baseline.exe` avant modification. Variante : remplacement des deux itérateurs scalaires de lignes (détection et parsing) par un itérateur de tranches utilisant `memchr` 2.8.3. Aucun changement du modèle, des règles du parsing ou du checksum, aucune allocation supplémentaire. Une dépendance externe ajoutée ; aucun unsafe ajouté au projet.

Comparaison alternée ancien/nouveau, cinq exécutions par version, `data --memory --repeat 10`, single-thread, répétitions groupées par fichier :

| Version | MB/s médian | Min–max MB/s |
|---|---:|---:|
| Ancienne | 357.73 | 340.01–368.12 |
| memchr | 493.97 | 470.18–495.35 |

Gain de débit médian : 38.1 % (réduction du temps d'environ 27.6 %). Le gain touche aussi le parsing des lignes, pas seulement la détection des mains. Données brutes : `memchr-runs.json`. Chaque run : 671 270 mains, 3 490 280 joueurs, 5 251 540 actions, zéro erreur, checksum `4c9d8e06291f90e0`.

Un run de contrôle avec 4 workers donne 1 532.41 MB/s, avec les mêmes faits et checksum ; ce chiffre isolé ne constitue pas une médiane ni une comparaison contrôlée de scaling.

Le programme `examples/phases.rs` garde volontairement la détection scalaire comme référence (`scalar_detection_reference`) : cette phase ne mesure plus le détecteur de production optimisé. Les anciennes mesures de phases restent historiques.

## Parallélisme dans un fichier par blocs

Option explicite `--block-bytes 262144 --threads 4`, ancien mode par fichiers conservé par défaut. Fichier mesuré : `20260519_Colorado_real_holdem_no-limit.txt`, 2 730 430 octets, 3 079 mains, 18 474 joueurs, 27 046 actions. Cinq comparaisons alternées, en mémoire, 200 répétitions. Découpage et workers inclus une fois, répétitions regroupées par bloc.

| Mode | MB/s médian | Min–max MB/s |
|---|---:|---:|
| Single-thread | 445.35 | 433.90–458.30 |
| 4 workers, blocs de cible 256 KiB | 1311.86 | 1240.12–1370.61 |

Accélération médiane : 2.95×. Tous les compteurs et checksums sont identiques, aucune erreur. Données brutes : `block-runs.json`. L'avantage reflète aussi la localité des blocs et l'amortissement de la préparation ; ce n'est pas un résultat de lecture disque à froid.

Contrôle lecture + parsing, 100 répétitions, un run par mode : 367.17 MB/s single contre 657.74 MB/s blocs ; chaque répétition inclut lecture, découpage et création/jonction des workers.

Contrôle corpus complet, un passage en mémoire : mêmes 67 127 mains, 349 028 joueurs, 525 154 actions, zéro erreur, checksum `6e0fc167041cc1b0` dans les deux modes. Par fichiers : 1749.72 MB/s ; blocs fichier par fichier : 488.81 MB/s. Runs courts non statistiques, mais le mode blocs ne doit pas remplacer le mode par fichiers sur ce corpus de nombreux petits fichiers. Il reste opt-in. Aucun réglage automatique du scheduling ni pool persistant ajouté dans cette itération.

## Campagne automatisée et correction renforcée

Commande exécutée : `./scripts/benchmark.ps1 -Runs 3 -Repeat 10 -SkipBuild`, après compilation release contrôlée. 48 mesures, 16 configurations, un échauffement par configuration. Synthèse conservée dans `verified-matrix-summary.csv` ; résultats complets locaux sous `target/benchmarks/20261007-200651-823/`. Les paramètres par défaut du script sont 5 runs ; cette validation initiale en a utilisé 3.

Corpus courant : 536 fichiers, 56 318 967 octets, 67 127 mains. Médianes MB/s sur le dossier complet :

| Workers | En mémoire | Lecture + parsing |
|---|---:|---:|
| 1 | 489.03 | 318.75 |
| 2 | 862.72 | 529.74 |
| 4 | 1598.06 | 815.27 |
| 8 | 2618.69 | 888.82 |

Tous les compteurs et checksums concordent entre configurations d'un même jeu de données, aucune erreur. La montée en débit mémoire ne doit pas être assimilée au débit disque : le gain avec lecture est plus limité et les caches Windows sont chauds. Les mesures du fichier isolé sont particulièrement courtes avec seulement 10 répétitions ; elles servent à valider le protocole et méritent une campagne plus longue.

Correction apportée par les nouveaux tests : un montant avec point mais sans décimale, tel que `2.€`, est désormais rejeté. Aucun fait du corpus valide n'est modifié. Validation totale : 18 tests, dont fixture synthétique avec tous les faits attendus, all-in incomplet, montants dépassant u64, comparaison de tous les faits du corpus entre séquentiel et blocs parallèles. Clippy sans avertissement.



## Recherche des acteurs — 8 octobre 2026

Le remplacement de filter_map/max_by_key par une boucle courte a d'abord montré +3.17 % sur cinq paires alternées (`actor-runs.json`). Cependant, un nouveau test a révélé un défaut préexistant : les joueurs `A` et `A raises` peuvent rendre une ligne de relance de A ambiguë. La simple règle du plus long nom attribuait alors un suffixe incorrect à l'acteur.

La version finale vérifie le verbe d'action uniquement lorsque plusieurs noms correspondent à la ligne, puis conserve la correspondance valide la plus longue. Cela évite de vérifier tous les verbes dans le cas courant. La syntaxe et les montants restent validés par le parseur après sélection. Tests sur pseudos avec verbes, Unicode et ordre inversé des sièges.

Comparaison finale alternée, cinq runs par version, `data --memory --repeat 30`, single-thread :

| Version | MB/s médian | Min–max MB/s |
|---|---:|---:|
| Avant | 565.91 | 562.50–569.38 |
| Boucle avec correction des ambiguïtés | 564.14 | 557.40–568.38 |

Différence : -0.31 %, trop faible et distributions trop proches pour conclure à un changement de débit. La modification est conservée pour la correction, pas comme un gain de performance. Tous les runs : 2 013 810 mains, 10 470 840 joueurs, 15 754 620 actions, zéro erreur, checksum `e5d8aa127b5eb2a0`.

Données finales : `actor-final-runs.json`. Les fichiers `actor-runs.json` et `actor-corrected-runs.json` documentent les variantes intermédiaires, qui ne sont pas le code livré. La vérification systématique des verbes avait ralenti le parsing d'environ 10 % ; elle a été remplacée par le traitement des seules ambiguïtés. Aucun nouveau profil CPU capturé ; les pourcentages de CPU-PROFILE.md restent ceux de la version précédente.

## Recherche des séparateurs avec memmem — 8 octobre 2026

`split` utilise `memchr::memmem::find` au lieu de `windows(...).position(...)`. La dépendance memchr était déjà présente. Aucune allocation, aucun unsafe ni changement de règles ajouté ; première occurrence, erreur si absent et séparateur interne non vide conservés.

Première comparaison (avant ajout de l'assertion explicite du contrat non vide) : 553.33 → 601.38 MB/s en médiane, +8.68 %, `split-memmem-runs.json`.

Comparaison du binaire final, cinq paires alternées, single-thread, `data --memory --repeat 30` :

| Version | MB/s médian | Min–max MB/s |
|---|---:|---:|
| Avant | 528.98 | 506.27–541.04 |
| memmem final | 590.29 | 564.24–597.61 |

Gain médian observé : 11.59 %. Les cinq paires favorisent la variante. Les niveaux absolus ont dérivé pendant les campagnes ; la comparaison alternée dans chaque campagne est plus pertinente que les chiffres historiques. Les deux campagnes soutiennent un gain de l'ordre de 9–12 % sur ce corpus, sans garantie universelle.

Tous les runs : 2 013 810 mains, 10 470 840 joueurs, 15 754 620 actions, zéro erreur, checksum `e5d8aa127b5eb2a0`. Résultats finaux : `split-final-runs.json`.

20 tests passent, dont test différentiel de séparateurs et comparaison de tous les faits entre séquentiel et blocs sur le corpus privé ; Clippy et formatage passent. Le test différentiel couvre octets arbitraires, absence, début/milieu/fin, séparateurs tronqués, répétitions et première occurrence. La variante est conservée. Aucun nouveau profil CPU enregistré pour cette version ; le profil précédent sert à identifier la piste et non à affirmer la nouvelle répartition des coûts.

## Validation du code actuel et passage à l'échelle — 8 octobre 2026

### Corpus réel

Campagne complète : `./scripts/benchmark.ps1 -Runs 5 -Repeat 30`, 80 mesures vérifiées. Sorties complètes sous `target/benchmarks/20261008-133033-844/`, synthèse partageable `current-matrix-summary.csv`.

| Workers | Mémoire MB/s médian | Lecture + parsing MB/s médian |
|---|---:|---:|
| 1 | 552.64 | 369.30 |
| 2 | 1086.79 | 815.74 |
| 4 | 2016.21 | 1442.94 |
| 8 | 2932.31 | 2099.84 |

Corpus réel de 536 fichiers, 56 318 967 octets. Mêmes compteurs et checksum pour toutes les configurations comparables. Cache système chaud, répétitions groupées par fichier. Ce n'est pas un benchmark disque à froid ni une comparaison directe avec un ancien run réalisé dans d'autres conditions.

### Fichiers synthétiques

Commande : `./scripts/scale.ps1`, trois runs pour chacune des 12 configurations, 36 mesures vérifiées. Base : concaténation des 536 fichiers, un newline ajouté par fichier, soit 56 319 503 octets. Copies entières de cette base (5, 20 et 39) pour préserver toutes les mains. Identifiants répétés, pas de mains nouvelles ni uniques. Sorties complètes : `target/scale/20261008-133244-827/`, synthèse : `scale-summary.csv`.

| Taille réelle | Mains | Mémoire, 1 worker MB/s | Mémoire, 4 workers blocs MB/s | Lecture, 1 worker MB/s | Lecture, 4 workers blocs MB/s | Pic working set observé |
|---|---:|---:|---:|---:|---:|---:|
| 281 597 515 octets | 335 635 | 511.55 | 1261.10 | 429.13 | 819.84 | 274 MiB |
| 1 126 390 060 octets | 1 342 540 | 515.61 | 1234.22 | 429.98 | 833.69 | 1079 MiB |
| 2 196 460 617 octets | 2 617 953 | 427.21 | 1213.74 | 360.05 | 795.36 | 2100 MiB |

Débits médians, un passage par run, blocs de cible 1 MiB avec 4 workers. Toutes les mains/joueurs/actions sont comparés à la base multipliée par le nombre de copies, le checksum est vérifié modulo 2^64, aucune erreur. Les empreintes SHA-256 des fichiers et du binaire sont enregistrées. Les fichiers sont fraîchement écrits puis relus : cache chaud, pas de débit disque à froid.

Mémoire : high-water mark du working set Windows interrogé toutes les 50 ms pendant la vie du processus ; le dernier intervalle peut manquer. Préchargement inclus même en mode mémoire, générateur PowerShell exclu. Ce n'est pas une mesure du heap seul. Le pic est proche de la taille du fichier plus quelques MiB et n'est pas multiplié par quatre en mode blocs.

Conclusion : aucune erreur au-delà de 2 GB, débit parallèle soutenu, mais mémoire linéaire avec la taille du fichier. La baisse single-thread à la plus grande taille est observée ; sa cause n'a pas été isolée. Prochaine évolution prioritaire : lecture par buffers réutilisables avec conservation de la main chevauchant une frontière, puis comparaison avec cette référence. Aucun streaming n'a été ajouté pendant ces essais.

## Lecture par buffers réutilisables — 8 octobre 2026

Implémentation de `scan_reader`/`process_reader` et de `--stream-bytes`. Le parseur et les conventions de faits sont partagés avec le mode intégral. Buffer réutilisé, conservation de la dernière main jusqu'à la prochaine frontière ou EOF, reconnaissance des en-têtes même coupés entre lectures. Mémoire O(buffer + plus grande main ou préambule), sans plafond absolu pour les entrées malformées.

Comparaison sur le fichier synthétique de 2 196 460 617 octets, 2 617 953 mains, trois runs par mode avec ordre tournant, single-thread, lecture comprise et cache Windows chaud :

| Mode | MB/s médian | Min–max MB/s | Pic working set observé |
|---|---:|---:|---:|
| Lecture intégrale | 364.96 | 353.83–365.90 | 2099.68 MiB |
| Streaming 1 MiB | 383.71 | 376.09–387.21 | 7.01 MiB |
| Streaming 4 MiB | 386.41 | 373.54–387.51 | 12.99 MiB |

La réduction de mémoire est nette ; le petit gain de débit est propre à cette mesure et ne garantit pas une accélération partout. Le buffer de 1 MiB est un compromis raisonnable observé, pas un optimum démontré. Les compteurs, octets et checksums concordent sur les neuf runs, aucune erreur.

Données : `stream-runs.csv`, `stream-summary.csv`. Métadonnées locales : `target/stream-bench/20261008-135708-379/`. Le pic est le high-water mark Windows interrogé toutes les 50 ms, potentiellement incomplet sur le dernier intervalle ; ce n'est pas une mesure du heap seul.

25 tests passent et Clippy/formatage passent. Nouveaux cas : chunks de 1 à 4096 octets, BOM/CRLF, fragments d'en-têtes, EOF/troncatures, octets parasites, égalité des faits et offsets, lectures courtes/interrompues, propagation des erreurs I/O, main plus grande que le buffer, CLI single/multi par fichiers et flags incompatibles. Le corpus réel complet est également comparé en lecture intégrale et streaming avec buffer de 4096 octets.

Streaming parallèle à l'intérieur d'un même fichier non implémenté : le parallélisme actuel reste par fichiers. Les modes existants sont conservés, le streaming est opt-in.

## Robustesse et préparation de diffusion — 8 octobre 2026

Streaming limité par défaut à 8 MiB par main/préambule brut, configurable avec `--max-hand-bytes`. La limite inclut espaces et BOM, et s'applique aussi aux mains complètes présentes dans une même lecture. Le buffer demandé reste borné par la limite plus 16 octets de chevauchement, même si la taille de lecture configurée est immense. Une entrée infinie sans frontière est rejetée après un nombre borné d'octets lus. Dépassement fatal explicite ; aucune main tronquée silencieusement.

Validation locale : 28 tests réussis, dont limites exactes, franchissement entre lectures, main trop grande au milieu d'un buffer, préambule hostile et options CLI. 100 000 mutations reproductibles : aucune panique ni divergence de faits/offsets entre lecture intégrale et streaming. La cible libFuzzer a également été compilée sur Windows ; l'exécution instrumentée Linux reste prévue en CI et n'a pas encore été confirmée.

Le workflow GitHub Actions couvre Windows/Linux (format, Clippy, tests release, mutations et fixture de streaming), avec un job Linux nightly de fuzzing guidé par couverture. Aucun distant n'est configuré et aucun run GitHub n'a été lancé depuis cette session.

La licence reste volontairement en attente. `publish = false` est activé. Une liste d'inclusion Cargo explicite exclut data/, target/ et les corpus/traces de fuzz locaux ; `cargo package --list --allow-dirty` a été contrôlé et contient 36 fichiers autorisés à cette étape. Voir RELEASING.md pour les conditions restantes avant diffusion. Les mesures de débit précédentes décrivent le code antérieur à cette limite ; aucun gain de performance n'est revendiqué pour la protection.

## Validation Linux locale — 8 octobre 2026

Sous Ubuntu WSL2 : 28 tests release, formatage et Clippy passent avec Rust stable et nightly. 100 000 mutations déterministes passent avec nightly. Le fuzzing instrumenté a ensuite exécuté 1 513 839 entrées en 301 secondes sans incident. Versions, périmètre et limites : `LINUX-VALIDATION.md`. Le script `scripts/validate-linux.sh` permet de relancer la validation. Aucun benchmark Linux natif ni run GitHub Actions n'est revendiqué.

## Mesures avec protection du streaming — 8 octobre 2026, soirée

Release Windows, Rust 1.90.0, CPU Intel Family 6 Model 154, 20 processeurs logiques. Binaire SHA256 `59EE8164BDF96B9ABA4252ED0C6189F74A43454AB8EC965A4E48C5267A4D03A0`. Cinq runs par configuration ; matrice avec 30 répétitions et échauffement, ordre tournant, cache système chaud. Corpus inchangé : 536 fichiers, 56 318 967 octets, 67 127 mains, 349 028 joueurs, 525 154 actions par répétition. Aucune erreur ni divergence de compteurs/checksum sur les 80 mesures de matrice.

| Corpus réel, médiane MB/s décimaux | 1 thread | 2 threads | 4 threads | 8 threads |
| --- | ---: | ---: | ---: | ---: |
| Lecture + parsing | 499,09 | 923,04 | 1 687,42 | 2 582,53 |
| Préchargé en mémoire | 620,10 | 1 199,35 | 2 307,18 | 3 834,33 |

Sur le fichier synthétique de 2 196 460 617 octets (39 copies du corpus, aucune nouvelle main unique), single-thread lecture + parsing, cinq runs par mode :

| Mode | Médiane MB/s | Min–max MB/s | Pic mémoire observé, MiB |
| --- | ---: | ---: | ---: |
| Lecture intégrale | 451,36 | 450,09–456,67 | 2 099,69 |
| Streaming 1 MiB | 499,91 | 488,12–506,34 | 5,99 |
| Streaming 4 MiB | 500,61 | 488,80–501,98 | 8,99 |

Les 15 mesures concordent : 2 617 953 mains, 13 612 092 joueurs, 20 481 006 actions, zéro erreur, checksum `c46676b1a06181d0`. SHA256 de l'entrée : `642652108D2808EEA12F6F31C045D27680077A99860D07F24E6E2909EE8F7F00`.

Dans cette campagne, 1 MiB offre un débit pratiquement identique à 4 MiB avec moins de mémoire. Le pic est le high-water mark du working set Windows interrogé toutes les 50 ms, avec possibilité de manquer le dernier intervalle ; ce n'est pas le heap seul. Le cache n'a pas été vidé. Les résultats sont supérieurs aux mesures historiques, y compris pour la lecture intégrale dont le chemin n'a pas été modifié : cet écart entre sessions ne démontre donc pas un gain causé par la protection. Aucun ralentissement n'est apparent dans cette campagne ; isoler le coût de la protection demanderait une comparaison A/B alternée de deux binaires.

Données conservées : `guarded-matrix-runs.csv`, `guarded-matrix-summary.csv`, `guarded-stream-runs.csv`, `guarded-stream-summary.csv` (culture CSV fr-FR). Métadonnées locales : `target/benchmarks/20261008-221037-760/` et `target/stream-bench/20261008-221155-814/`. Aucun changement du parseur pendant ces mesures.
