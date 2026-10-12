# Independent source-only review

All 27 source answers, 27 target fields, and 33 evidence rows were checked
independently against the original workbook's exact I/J and M/N cells and the
publisher's `BROKER_SUBAWARDS_COLUMNS` type declaration. All 21 positive
correspondences are exact documented crosswalk aliases. Each of the six
no-match concepts has its counterpart absent from the target projection.

The distinctions supporting abstention are prime versus subrecipient award
amounts, prime versus subrecipient business classifications, a possibly nonunique
subaward tracking number versus an FSRS report identifier, executive annual
compensation versus an award amount, subrecipient legal street versus performance
or prime-recipient street, and subrecipient legal ZIP versus performance or
prime-recipient ZIP.

The review verified that no geography code/name alternatives were mislabeled as
different facts and that no numbered-officer identity or ranking assumption was
introduced. The report-last-modified/date-submitted positive and the subaward
performance-street distractor follow the publisher crosswalk even where database
names are misleading. No blocking semantic or extraction defect was found.

All target types preserve declared PostgreSQL `TEXT` as `text`; CSV storage types
remain `unknown` where not declared. All sample arrays are empty. Comparing only
the previously sourced first-tranche labels found no reused source names or
answer pairs. Copied-tree offline extraction regenerated both outputs byte for
byte. No runtime matcher implementation, candidate policy, reports, predictions,
or first-tranche runtime messages were accessed during this work.
