# PokerForge — benchmark de parsing

Prototype Rust synchrone, avec une dépendance ciblée (`memchr`, recherche des fins de ligne et des séparateurs). Un seul format : Winamax CashGame et ESCAPE, Holdem no limit, montants EUR avec au plus deux décimales. Le fichier de référence est `data/20260620_Aalen 12_real_holdem_no-limit.txt`.

## Utilisation

```powershell
cargo test
cargo build --release
# Fichier de référence, single-thread
.\target\release\pokerforge.exe 'data/20260620_Aalen 12_real_holdem_no-limit.txt'
# Tous les TXT, récursivement, lecture comprise
.\target\release\pokerforge.exe data
.\target\release\pokerforge.exe data --threads 4
# Parsing en mémoire, lecture initiale exclue du chronomètre
.\target\release\pokerforge.exe data --memory --repeat 100
.\target\release\pokerforge.exe data --memory --repeat 100 --threads 4
```

Le mode par défaut est single-thread. `--threads N` distribue les fichiers à des workers synchrones. Une opération atomique par fichier distribué, des statistiques locales, une réduction à la fin ; aucun Mutex ou dictionnaire global. Le nombre de workers est plafonné au nombre de fichiers. Le mode optionnel par blocs est décrit ci-dessous.

`Files` compte les chemins uniques. Bytes, Hands, Players, Actions, Parse errors et Checksum incluent les répétitions. Hands compte les mains acceptées ; Players est la somme des sièges déclarés, pas le nombre de personnes distinctes. MB/s utilise 1 MB = 1 000 000 octets. Les erreurs rendent le code de sortie non nul ; les dix premières sont affichées avec chemin, offset en octets et raison après le chronométrage. Les offsets sont relatifs au contenu après un éventuel BOM UTF-8.

## Modèle et conventions

- `HandFacts` et son tableau fixe de 10 `PlayerFacts` vivent sur la pile. Aucun heap dans `parse_hand`, `scan` ou `process`. Les noms restent des tranches empruntées au buffer, uniquement pendant le parsing. Pas de regex, String, serde ou allocation par ligne/main.
- Montants `u64` en centimes. `player_id` = numéro de siège, local à cette main ; ce n'est pas une identité persistante.
- `hand_id` = FNV-1a 64 bits sur l'identifiant Winamax complet, tirets inclus. Déterministe, mais risque de collision : un futur tracker devra conserver l'identifiant source pour garantir l'unicité.
- Timestamp = troisième composante numérique de l'identifiant Winamax (secondes Unix dans le corpus). La date textuelle n'est pas convertie ni recoupée.
- Actions : fold, check, call, bet, raise. Exclut posts/antes, cartes montrées, gains et lignes récapitulatives.
- VPIP : call/bet/raise préflop ; les mises forcées ne comptent pas. PFR : raise préflop. Three-bet : deuxième événement `raises` préflop, même si all-in incomplet ; ce n'est pas une analyse des règles de réouverture des enchères.
- Board : 0 à 5 cartes codées sur un octet (`1 + rang * 4 + couleur`, rangs `23456789TJQKA`, couleurs `cdhs`). Progression et récapitulatif recoupés, doublons refusés.
- Checksum : mélange de tous les faits extraits, puis somme modulo 2^64 entre mains, donc indépendant de l'ordre des workers. `black_box` consomme les faits. Ce checksum n'est pas cryptographique.

## Portée de la correction

Tests synthétiques sans données privées : faits attendus, noms avec préfixes communs, VPIP/PFR/3-bet, all-in, CRLF/LF/BOM, EOF sans newline, montants invalides, acteur inconnu, cartes dupliquées, ordre des streets, main incomplète, récupération à la main suivante, mutations/troncatures et équivalence CLI single/multi en mémoire et avec lecture. Le test du corpus privé est exécuté si ce fichier existe. Les données `data/` sont ignorées par Git.

Le parseur valide la syntaxe nécessaire aux faits, les montants des actions et la structure principale. Ce n'est pas un arbitre de poker : il ne vérifie pas la légalité de chaque action, la conservation des pots ou l'équité. Les détails de `Dealt to`, `shows` et des lignes Seat du résumé ne sont pas intégralement validés. Une troncature avant le total du résumé est rejetée ; une coupure après les éléments requis du résumé n'est pas toujours détectable. Aucun support annoncé pour tournois, autres devises, variantes ou run-it-twice.

## Méthode de mesure

Voir `benches/RESULTS.md`. Single-thread implémenté, testé et mesuré avant l'ajout du parallélisme. Les répétitions sont effectuées par fichier : elles favorisent volontairement les caches CPU. Ce sont des mesures de débit à chaud ; elles ne prouvent ni le plafond matériel absolu ni une supériorité sur des trackers complets.

En mode mémoire, tous les fichiers sont chargés avant le chrono. En lecture + parsing, un buffer par fichier et par répétition est alloué, puis libéré : mémoire bornée approximativement par les plus gros fichiers traités simultanément. La découverte/tri des chemins est exclue du chrono dans les deux modes. Le temps inclut scan des mains, parsing, checksum, agrégation et, en multi-thread, création/jonction des workers. Pour de très gros fichiers, une lecture par blocs serait la prochaine évolution à mesurer.

## Corpus élargi

Les en-têtes ESCAPE sont acceptés explicitement. La ligne optionnelle `Escape to Pot: total ...` est validée en euros puis ignorée pour les faits (ce supplément de pot ne constitue ni une action ni du VPIP).

Pour obtenir la liste complète des rejets par fichier et par raison, hors benchmark :

```powershell
cargo run --release --example audit -- data
```

Les 17 fichiers de tournois et leurs 17 résumés ont été retirés sur demande. Le corpus courant contient 536 fichiers, 67 127 mains et zéro erreur ; ces formats restent explicitement rejetés si réintroduits. Les résultats de débit de ce corpus mixte incluent le coût des rejets et ne représentent pas exclusivement du parsing de mains acceptées.

Pour isoler le coût de la détection des mains, le parsing précalculé et le pipeline complet :

```powershell
cargo run --release --example phases -- data
```

Cet outil exclut et compte les fichiers avec erreurs avant la mesure ; il effectue cinq runs de vingt passages du corpus et fait tourner l'ordre des phases. Il ne remplace pas un profileur CPU. Voir les limites d'interprétation dans `benches/RESULTS.md`.


## Gros fichier unique : workers par blocs

```powershell
.\target\release\pokerforge.exe 'data/20260519_Colorado_real_holdem_no-limit.txt' --memory --threads 4 --block-bytes 262144 --repeat 200
```

`--block-bytes N` active explicitement le découpage aux frontières des mains. La taille cible est un minimum souple : la frontière suivante est choisie, une main reste toujours entière. Les blocs empruntent le buffer source, sans copie du contenu ; un tableau de plages est alloué une fois par découpage. Un compteur atomique distribue les blocs, chaque worker agrège ses faits localement.

Ce mode traite les fichiers successivement, avec plusieurs workers dans chacun. Le mode par fichiers reste le défaut pour un répertoire de petits fichiers. `Workers` indique le maximum de workers utilisés pour un fichier, pas une moyenne. Une entrée vide ou ne permettant qu'un bloc utilise un worker.

En mémoire : lecture initiale exclue, découpage et création/jonction des workers inclus dans le chrono ; chaque bloc est répété N fois, donc les frais de préparation sont amortis sur les répétitions et les caches favorisés. Avec lecture : chaque fichier est relu, redécoupé et traité à chaque répétition ; un seul buffer de fichier est vivant à la fois. Ce n'est pas une lecture streaming : la mémoire reste proportionnelle au fichier chargé.

La reconstruction des octets et l'équivalence des faits/erreurs sont testées sur plusieurs tailles de blocs, BOM, CRLF, texte parasite, main tronquée et fichier vide. Un test CLI vérifie quatre workers sur un fichier unique, dans les deux modes de lecture.

## Validation et campagne reproductible

Depuis PowerShell 7, à la racine du projet :

```powershell
cargo test --release
./scripts/benchmark.ps1
# Campagne plus longue :
./scripts/benchmark.ps1 -Runs 7 -Repeat 100
```

Le script compile avec `--locked` puis mesure 1/2/4/8 workers, lecture + parsing et mémoire, sur le corpus entier (par fichiers) et son plus gros fichier (par blocs au-delà de 1 worker). Il effectue un échauffement par configuration, fait tourner leur ordre entre runs et refuse toute erreur ou divergence de compteurs/checksum. Les répétitions du fichier isolé peuvent être trop courtes pour conclure : augmenter `-Repeat` et consulter la dispersion.

Chaque campagne crée un dossier unique sous `target/benchmarks/` avec :

- `corpus.json` : chemins relatifs, tailles et SHA-256 de tous les fichiers ; corpus revérifié après les mesures.
- `metadata.json` : empreinte du corpus, du binaire et des sources Rust/tests/scripts/Cargo, version Rust, état Git, OS, CPU déclaré et nombre de processeurs logiques, paramètres.
- `runs.csv` : mesures individuelles et compteurs.
- `summary.csv` : médiane, minimum, maximum et écart-type de population des débits par configuration.

Les caches du système restent chauds ; aucune prétention de lecture à froid. `-SkipBuild` est réservé à un binaire déjà compilé : son SHA-256 est enregistré séparément des sources, car le script ne peut pas garantir leur correspondance. Aucun historique privé n'est copié dans les résultats ; le manifeste contient néanmoins les noms de fichiers et reste dans `target/`, ignoré par Git.

La fixture `tests/fixtures/split-pot.txt` est entièrement synthétique. Tous ses `HandFacts` attendus sont explicitement vérifiés : identifiant, timestamp, blindes, actions, board, stacks, sièges et flags. Les tests figent également la convention du deuxième `raises` pour les all-in incomplets. Une comparaison de faits complets entre tranches séquentielles et parallèles couvre le corpus privé lorsqu'il est disponible ; elle vérifie le découpage, pas indépendamment la justesse des règles du parseur. Les tests du corpus privé restent facultatifs hors de cette machine.

## Profil CPU Windows (WPR)

Le profil Cargo `profiling` conserve les optimisations release et ajoute les symboles (`debug = 2`). Depuis **PowerShell administrateur**, à la racine du projet :

```powershell
./scripts/profile-cpu.ps1
```

Le script compile, démarre une capture WPR CPU, exécute le corpus en mémoire en single-thread puis arrête la capture dans un bloc `finally`. Il conserve ETL, sortie du benchmark, métadonnées et copie du binaire/PDB correspondants sous `target/profiles/`. Si le démarrage échoue, il ne tente pas de stopper une session existante. Une interruption brutale du terminal peut empêcher le `finally` : vérifier alors `wpr -status`.

La trace contient des échantillons CPU système ; garder ce fichier local. Dans WPA, ouvrir `cpu.etl`, charger les symboles du dossier de capture, filtrer CPU Usage (Sampled) sur `pokerforge.exe` et examiner les piles/fonctions. L'inlining et le LTO peuvent regrouper le coût de plusieurs fonctions : distinguer temps propre et inclusif.

Tentative depuis cette session : compilation réussie, capture refusée par Windows (`0xc5585011`, privilège de profilage système). Une capture a ensuite été réalisée dans un terminal administrateur et analysée : voir benches/CPU-PROFILE.md et scripts/analyze-cpu.ps1.


La recherche des acteurs utilise une boucle sans allocation. Lorsque plusieurs pseudos correspondent à une ligne, elle vérifie le verbe suivant le nom pour départager des cas comme `A` et `A raises`, puis conserve le nom valide le plus long. Les tests couvrent aussi les déclarations de sièges dans un ordre inversé. Cette correction conserve les faits du corpus actuel et n'apporte pas de gain de débit significatif mesuré.

La fonction `split` utilise désormais `memchr::memmem::find` pour trouver la première occurrence du séparateur, sans allocation. Un test différentiel compare les tranches renvoyées à la recherche scalaire de référence, sur des séparateurs présents/absents, en début/fin de buffer, répétés et sur des octets arbitraires. Le contrat interne exige un séparateur non vide, comme auparavant.

## Gros fichiers synthétiques et mémoire

```powershell
./scripts/scale.ps1
```

PowerShell 7 sous Windows. Utilise le binaire release existant : compiler avant avec `cargo build --release --locked`. Par défaut, le script concatène le corpus courant, enlève un éventuel BOM par fichier et ajoute une fin de ligne entre fichiers. Il répète cette base entière pour atteindre au moins 256 MiB, 1 GiB et 2 GiB, sans tronquer une main. Les tailles effectives peuvent donc dépasser les cibles. Les identifiants sont répétés : ce n'est pas un corpus de mains uniques ni un test de déduplication.

Trois runs par configuration : single-thread/par blocs à 4 workers, lecture/mémoire, un passage par run. L'ordre tourne entre runs. Les fichiers fraîchement écrits et relus bénéficient du cache Windows ; aucune mesure de disque à froid. Les compteurs attendus sont ceux de la base multipliés par son nombre de copies ; le checksum attendu est calculé modulo 2^64.

Les données synthétiques, leurs empreintes et les résultats restent dans un dossier unique sous `target/scale/`, ignoré par Git. Prévoir environ 3.6 GB de disque par campagne par défaut. La génération utilise la base en mémoire, mais la mesure mémoire concerne uniquement le processus Rust enfant. Le pic de working set rapporté par Windows est interrogé toutes les 50 ms : la dernière fraction de seconde peut échapper à la mesure. Ce n'est ni un compteur d'allocations, ni une mesure exacte du heap, ni la consommation totale de toute la campagne. Le préchargement est inclus dans cette mesure mémoire même avec `--memory`, bien qu'il soit exclu du chronométrage de parsing.

## Lecture incrémentale

```powershell
# Un fichier ou un dossier, single-thread par défaut
cargo run --release -- data --stream-bytes 1048576
# Plusieurs fichiers simultanés, un buffer réutilisable par worker actif
cargo run --release -- data --stream-bytes 1048576 --threads 4
```

Ce mode lit par blocs de N octets et conserve la dernière main incomplète entre les lectures. Les frontières sont reconnues uniquement en début de ligne ; l'en-tête peut chevaucher deux lectures. Le même parseur extrait les faits. Le buffer est réutilisé, sans allocation par ligne/main ; le reliquat est déplacé en tête entre lectures. La recherche de frontière ne reparcourt pas tout le reliquat lorsqu'une main est très grande.

La mémoire est O(buffer + plus grande main/préambule), par fichier actif. Ce n'est **pas une borne absolue face à une entrée malformée sans nouvelle frontière**. Le buffer peut grandir pour préserver une main dépassant sa taille, sans tronquer silencieusement les données. Une limite configurable des mains/entrées malformées reste une évolution distincte.

`--stream-bytes` est incompatible avec `--memory` et `--block-bytes`. Le mode multi-thread distribue les fichiers ; un fichier unique en streaming utilise un worker. Les diagnostics d'erreurs sont eux aussi relus en streaming, pour éviter de recharger tout le fichier en cas d'échec. Les offsets suivent la convention existante : après un éventuel BOM initial.

Pour reproduire la comparaison mémoire/débit sur un gros fichier :

```powershell
./scripts/benchmark-stream.ps1 -InputFile 'target/scale/20261008-133244-827/synthetic-2048-MiB.txt'
```

Le chemin ci-dessus correspond au fichier synthétique local de cette campagne ; remplacer par un fichier existant. Le script compare lecture intégrale, buffers de 1 et 4 MiB, en single-thread avec lecture comprise. Il vérifie compteurs et checksum, enregistre SHA-256 du fichier/binaire et observe le pic de working set toutes les 50 ms. Les mêmes limites de cache et de mémoire que la campagne de passage à l'échelle s'appliquent.

## Limite de sécurité du streaming et statut de diffusion

Le streaming impose maintenant une limite de **8 MiB par main ou préambule brut**, espaces et BOM compris. Cette règle remplace l'absence de plafond décrite dans les mesures historiques ci-dessus. Elle inclut les blancs avant l'en-tête suivant. La taille de lecture effective est plafonnée pour ne pas dépasser la limite plus les 16 octets de chevauchement nécessaires à reconnaître la frontière suivante ; le buffer peut être plus petit que la taille de lecture demandée.

```powershell
cargo run --release -- data --stream-bytes 1048576 --max-hand-bytes 16777216
```

La limite est configurable, positive et propre au streaming. Un dépassement arrête le fichier avec une erreur contenant la limite et un offset brut (BOM inclus), et le programme termine en échec. Les éventuels faits déjà traités avant l'erreur sont partiels ; aucune réussite globale n'est annoncée. Les erreurs de parsing ordinaires conservent leur comptage et leurs offsets après BOM. Il n'y a pas de récupération silencieuse d'une main trop grande.

La taille maximale du buffer demandé est liée à la limite choisie, indépendamment de la taille du fichier. Avec plusieurs workers, ce budget est par fichier actif. Il ne s'agit pas d'une limite sur la mémoire totale du système ou sur les métadonnées de découverte des fichiers.

Ce projet est sous **GNU GPL version 3 uniquement** (`GPL-3.0-only`) ; voir [LICENSE](LICENSE) pour le texte intégral. Les dépendances conservent leurs licences respectives. Dépôt public : [Bertollo-Nicolas/PokerForge](https://github.com/Bertollo-Nicolas/PokerForge). La publication Cargo reste désactivée ; voir `RELEASING.md` pour les étapes de release.

Dernière campagne Windows avec cette limite active : sur le fichier synthétique de 2,2 Go, streaming single-thread à **500 MB/s environ**, pic résident observé **6 MiB** avec buffer de 1 MiB (cinq runs, cache chaud). Sur le corpus réel : **499 MB/s** en lecture + parsing avec un thread, **2 583 MB/s** avec huit threads. Compteurs et checksums concordants, zéro erreur. Méthode, dispersion et limites dans `benches/RESULTS.md` ; ces valeurs ne constituent pas un plafond universel.

Un parseur de référence indépendant, réservé aux tests, compare chaque champ avec le parseur rapide. Huit scénarios synthétiques possèdent des résultats attendus explicites et dix corruptions ciblées doivent être rejetées. Sur le corpus privé local, les 67 127 mains concordent. Commande et périmètre : `tests/REFERENCE.md`. Ce contrôle ne valide pas toutes les règles de légalité du poker.
