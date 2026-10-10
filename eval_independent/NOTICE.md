# Data and document notices

These evaluation assets are separate from Fieldkin's Rust library and excluded
from its published Cargo package. The root library license does not replace the
publisher terms below. All downloaded files retain their original bytes and
notices; the frozen manifest identifies the terms and source of each file.
The schema JSON files are projections made on 2026-10-10, not publisher releases.

* **Palmer penguins:** Kristen Gorman, Palmer Station LTER, Allison Horst,
  Alison Hill and Kristen Gorman's `palmerpenguins` 0.1.1. CC0 1.0;
  full terms in `sources/palmer_license.md`. Original study: Gorman, Williams,
  Fraser (2014), *PLoS ONE* 9(3), e90081, doi:10.1371/journal.pone.0090081.
  The archived transform specifies its EDI editions; newer documentation cites
  later EDI editions. We use the pinned archive, not those later downloads.
* **nycflights13:** Hadley Wickham/Posit, version 1.0.2.9000 at the pinned
  publisher revision. CC0 as explicitly declared in `sources/nyc_description.txt`.
  Original providers are US BTS/RITA, US FAA and Iowa Environmental Mesonet.
  Only flights, planes, airlines and weather are included. OpenFlights airports
  were excluded because their separate licensing was not established here.
* **R datasets:** R Core Team, development-tree revision recorded in the manifest;
  documentation distributed under GPL 2 or later. Full GPL 2 text is in
  `sources/r_license.txt`; original R constructors and manuals accompany the
  CSV projections. R-derived corpus portions retain those terms.
  `Rdatasets` is only an export mirror: its repository's code license is not
  treated as a license grant for the datasets. Dataset-specific original
  citations and caveats are in the archived R manuals.
* **quantreg Engel:** Roger Koenker et al., quantreg 6.1, GPL >= 2 according to
  `sources/quantreg_description.txt`. The original native data and manual are
  retained. Statsmodels independently declares the underlying Engel data public
  domain. Original household observations: Engel (1857); Koenker and Bassett
  (1982), *Econometrica* 50, 43-61.
* **AER Grunfeld:** Christian Kleiber and Achim Zeileis, AER 1.2-17,
  GPL-2 OR GPL-3 according to `sources/AER_description.txt`; GPL-2 is available
  in the included R license text. Original native data and manual are retained.
  Statsmodels independently declares the underlying Grunfeld data public domain.
  The original 11-firm edition is used, with the publisher's corrections/version
  caveats intact; see Grunfeld (1958) and Kleiber/Zeileis (2010).
* **Statsmodels:** the pinned publisher's dataset `data.py` files explicitly
  declare each included dataset public domain (Longley, Nile, sunspots, Engel,
  Grunfeld, stackloss and NOAA El Nino). Its code and documentation are BSD-3;
  full copyright/license notices are in `sources/sm_license.txt`.

Several original government/EDI endpoints were unavailable under the execution
environment's network policy. Acquisition used the recorded publisher GitHub
archives and, for R CSV exports, the pinned Rdatasets mirror. We do not claim
that the original endpoints were retrieved or that historic compilations are
newly collected observations. Permissions above are publisher declarations;
we did not independently audit the entire upstream licensing chain.
