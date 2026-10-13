# Existing semantic failure analysis

At baseline commit `c6b5a2c`, the two **previously exposed** reliability tranches
contain 236 fields: 118/138 correct automatic matches, 52/114 recovered renamed
fields, and 145/236 correct decisions. This analysis classifies all 20 incorrect
automatic matches and 62 missed renamed correspondences. Eight fields occur in
both groups, leaving 74 distinct fields. It provides diagnosis, not independent
validation or evidence that a proposed fix generalizes.

The per-field records and evidence-document hashes are in
[`semantic_failure_analysis.json`](semantic_failure_analysis.json). Classification
uses the unchanged inputs, complete original candidate arrays, archived publisher
definitions/crosswalks, and existing labels. No labels or baseline counts changed.
Primary categories are mutually exclusive; multiple mechanisms can contribute.

| Primary cause | Wrong automatic | Missed rename |
|---|---:|---:|
| Hierarchical leaf/parent role and camelCase words treated as flat names | 8 | 25 |
| Explicit code/description role ignored | 3 | 6 |
| Abbreviation/truncation with missing-evidence gate | 0 | 13 |
| Code/description role absent from supplied source input | 5 | 3 |
| Semantic/entity alias not established by flat signals | 0 | 14 |
| Genuine quantity scope missing from a generic name | 0 | 1 |
| Generic identifier alias without entity scope | 1 | 0 |
| Temporal representation changes penalized by type evidence | 2 | 0 |
| Unrelated fuzzy subsequence receives strong abbreviation evidence | 1 | 0 |
| **Total** | **20** | **62** |

Of the missed renames, 33 return NoMatch, 21 Ambiguous, and eight choose the wrong
target. The correct target already ranks first for 29 and second for 18. Improving
candidate generation alone cannot resolve most of these failures. Conversely,
changing acceptance policy alone cannot repair the 15 gold targets below rank two.

Examples with useful existing evidence:

* DCSA `transportCall.vessel.vesselOperatorCarrierCode` selects
  `transportCallReference`, despite a target with an `operatorCarrierCode` leaf.
  Long shared paths and unsplit camelCase obscure the distinctive terminal role.
* FHIR `period.start` selects `location.period.start` rather than
  `actualPeriod.start`; overall encounter time and time at a particular location
  share generic leaf names but different scope.
* GBFS `rental_uris.ios` selects `rental_uris.web`. Explicit platform names and
  corresponding sibling structure are available; flat whole-name similarity
  underuses them.
* Treasury `business_funds_indicator_description` selects its indicator code,
  despite a target ending in `ind_desc`. Description/code role words and common
  abbreviation structure are present.

Thirty-six missed renames have Unknown source types and no samples. With the
unchanged weights and minimum score, the missing-sample evidence gate requires
name similarity at least `119/120` when type evidence is 0.5. Correctly ranked
abbreviations often fall short. Relaxing this gate globally would also encourage
unsafe schema-only matches; stronger ranking is not semantic proof.

Five wrong automatic matches lack their distinguishing source role in runtime
inputs. The Treasury source names `cost_or_pricing_data`, `evaluated_preference`,
and `solicitation_procedures` denote descriptions; identically named targets denote
codes. `extent_competed` and `type_of_set_aside` similarly denote source
descriptions but choose target codes. Archived crosswalk definitions establish
that distinction; supplied fields have Unknown type and no values. Name equality
cannot recover absent facts. Likewise, source `amount` does not establish which
of several target quantity scopes is equivalent without its definition.

Useful contrasts exist in the supplied names/paths for 12/20 wrong automatic
matches and 44/62 missed renames. The remaining cases have partial evidence
(3 and 15) or missing role distinctions (5 and 3). These are human diagnoses,
**not guaranteed recoverable counts**: a different parent name can be either a
legitimate migration or a different concept, and an apparent abbreviation can be
accidental.

The smallest promising mechanisms are existing word-boundary normalization,
ordered parent/leaf evidence for hierarchical names, and acceptance based on
corroboration rather than whole-name similarity alone. Any retained candidate
must demonstrate precision/recall gains and preserve legitimate migrations.
Additional metadata is necessary where meaning is absent; field-name exceptions
would merely encode these benchmark answers.

All five Valentine implementations were inspected. COMA and Cupid use different
lexical evidence; Cupid adds WordNet relatedness, which is not equivalence (its
existing test gives dog/cat similarity above 0.8). DistributionBased and Jaccard
require observed values and cannot help the schema-only financial cases.
SimilarityFlooding builds a flat table/column/type graph, not the actual parent
paths embedded in names. None contains publisher migration definitions or units.
Their scores are not interchangeable probabilities, and agreement can repeat the
same lexical or sample bias. Complementary rankings merit measurement; blindly
averaging scores or vetoing a correct rename for lack of agreement is unjustified.

Publisher examples are illustrative, not production observations. This corpus
has no independently labeled gold ambiguities. Its Ambiguous predictions therefore
measure missed unique decisions or unnecessary abstention, not correctness on
truly ambiguous inputs. Both limitations remain material.
