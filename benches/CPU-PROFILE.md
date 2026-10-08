# Profil CPU par échantillonnage

Mise à jour : les symboles du runtime ont été résolus ; voir la dernière section pour les résultats complets.

Trace capturée le 7 octobre 2026, analyse terminée le 8 octobre. Binaire optimisé avec symboles (`profiling`, release + debug=2), single-thread, corpus en mémoire, 150 répétitions. 536 fichiers, 10 069 050 mains traitées, zéro erreur ; checksum `7d3b525c68d97d20`. Temps du benchmark : 18.561222 s. Son débit de 455.13 MB/s sous instrumentation ne remplace pas les benchmarks sans capture.

Le SHA-256 du binaire sauvegardé concorde avec les métadonnées : `764F7381F17D0B0EA9D1130A3AE6D6C5599916B0C136DE51E5BCEA0A43839CB5`. Symboles Rust décodés depuis le PDB sauvegardé, sans téléchargement de symboles externes. PID analysé : 5068. Export effectué avec `xperf -symbols -a profile -detail`.

18 444 échantillons pour le processus, poids total environ 18.470302 secondes. Pourcentages ci-dessous normalisés au processus, pas au CPU global. Attribution aux fonctions échantillonnées (temps propre), pas aux piles inclusives. LTO et inlining regroupent certaines opérations dans leur appelant.

| Fonction ou groupe | Part du poids |
|---|---:|
| vcruntime140.dll, fonctions non résolues | 23.39 % |
| parse_hand, opérations intégrées comprises | 22.28 % |
| memchr, trois fonctions de recherche combinées | 17.82 % |
| Recherche du joueur par filter_map / max_by_key | 8.99 % |
| cards | 7.35 % |
| money | 3.81 % |
| split | 3.57 % |
| memcmp dans le binaire | 3.34 % |
| callback process, consommation des faits et checksum | 2.76 % |
| process | 2.18 % |

La somme ne vaut pas 100 % : fonctions mineures et code système omis. Données complètes propres à pokerforge : `cpu-functions.csv`. Les noms Rust très longs de la chaîne d'itérateurs permettent de relier le poste de 8.99 % à la recherche du nom du joueur dans `parse_hand`.

## Décision pour les prochains essais

1. Résoudre les 23.39 % de vcruntime avant de leur attribuer une cause : une copie, une remise à zéro ou une comparaison sont des hypothèses, pas des résultats de ce profil.
2. Essai indépendant et mesurable sur la recherche des acteurs : une boucle courte ou une stratégie préservant le choix du plus long préfixe. Ne pas remplacer naïvement par le premier nom correspondant : les pseudos peuvent partager un préfixe.
3. Envisager ensuite le parcours fusionné des lignes. memchr apparaît encore souvent mais est appelé par la détection et par le parseur ; le profil plat ne donne pas leur répartition. Sa présence ne démontre pas que memchr lui-même est inefficace.

Les coûts money/split sont moins prioritaires dans ce profil. Aucun changement du parseur n'a été effectué pendant cette analyse. Un profil par échantillonnage n'est pas un oracle exact et les gains devront être confirmés par la matrice de benchmark et les tests de faits.

## Reproduction

```powershell
./scripts/analyze-cpu.ps1 -CaptureDirectory target/profiles/20261007-201442-749
```

Le script vérifie l'empreinte du binaire, utilise uniquement les symboles locaux, génère un cache sous le dossier de capture et normalise les poids. En présence de plusieurs processus pokerforge, fournir `-ProcessId`. Les traces système et l'export brut restent sous `target/`, ignoré par Git ; seul le tableau filtré pokerforge est conservé dans benches.

## Symboles du runtime résolus — 8 octobre 2026

Cette section remplace le constat de runtime inconnu ci-dessus. Les symboles publics Microsoft correspondant aux modules de la trace ont été téléchargés dans `target/profiles/microsoft-symbols`, puis l'export xperf a été recalculé. La trace et le binaire analysés restent ceux du 7 octobre, avant la correction des noms ambigus.

Sur le poids CPU total de pokerforge :

- `vcruntime140.dll!memcmp` : 21.708 %.
- Son thunk (relais d'appel) : 0.963 %.
- Le symbole `pokerforge.exe!memcmp` : 3.344 %.
- Les autres fonctions de vcruntime sont principalement des déplacements mémoire, pour environ 0.72 %.

Les comparaisons et leurs relais représentent donc environ **26.0 %** du poids échantillonné du processus. Il ne s'agissait pas principalement d'allocations ni de remises à zéro. Données mises à jour : `cpu-functions-resolved.csv` ; la table précédente est conservée comme export historique.

Un export des piles filtrées sur memcmp (`xperf -symbols -a stack -butterfly 30 -pid 5068 -symbol memcmp`) révèle les appelants directs du memcmp du runtime :

| Appelant | Hits inclusifs passant par cet appelant | Part des 3 983 hits inclusifs de ce memcmp |
|---|---:|---:|
| Recherche du joueur (ancienne chaîne filter_map / max_by_key) | 2421 | 60.78 % |
| split (recherche de séparateurs) | 1393 | 34.97 % |
| parse_hand, opérations intégrées | 112 | 2.81 % |
| Vérification de noms déjà présents | 57 | 1.43 % |

Ce tableau est un comptage de piles filtrées, pas le même dénominateur que les poids CPU de la table générale ; ne pas multiplier ces pourcentages pour fabriquer des temps précis. Les proportions de recherche de joueurs concernent l'ancienne implémentation. Une nouvelle capture serait nécessaire pour connaître le profil de la version corrigée.

Prochaine expérience indépendante justifiée : remplacer la recherche générique `windows(...).position(...)` de `split` par une recherche de sous-chaîne adaptée, puis mesurer. Cela cible les nombreuses comparaisons de séparateurs, avec une interface et des règles de parsing inchangées. Le coût des comparaisons ne peut pas être supprimé intégralement et ce profil ne garantit pas un gain de 26 %.
