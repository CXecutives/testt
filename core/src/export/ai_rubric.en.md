# Scoring rule

This rule applies to the check in an AI chat from the app and to the job-matching skill, which
reads the same rule in German. The profile wins: every threshold comes from the profile, and a
key the profile does not set switches its rule off.

## Points from 1 to 10

- 9 to 10 core field, all must-have requirements met, at least one focus area met
- 7 to 8 core field, small gaps or overqualified
- 5 to 6 one must-have requirement open, or a permanent role with open terms
- 3 to 4 several must-have requirements or a formal requirement open
- 1 to 2 outside the field

If the profile names no focus areas, the condition about the focus area does not apply. A
missing nice-to-have costs at most half a point. Distance costs nothing for an interim role.

## Caps

- One formal requirement open (degree, licence, certificate) at most 4
- At least two and at least half of the must-have requirements open at most 4
- One must-have requirement or one counted term open at most 6
- No must-have requirement met at most 3
- Nothing open at least 4
- A job that is shown at least 2

The location of an interim role does not count as a term.

## Contract type and temporary agency work

- Interim (day rate, freelance, contract for work, contract) is the first category.
- A permanent role costs about one point, unless the profile looks for permanent roles only.
- A staffing agency that gives no contract type is unclear. It is checked like a permanent
  role, and the risk of temporary agency work is named.
- Temporary agency work (in German ads Arbeitnehmerüberlassung, ANÜ or Überlassung, also
  payrolling, equal pay, iGZ, BAP) excludes a job if the profile excludes it. As one of several
  options it is an open point.
- An unclear contract type is a point to check, never an exclusion.

## Exclusion

- An exclusion needs a verbatim quote from the ad that proves it.
- Only hard criteria exclude. These are temporary agency work, a day rate below min_tagessatz,
  a country outside laender, a salary below min_jahresgehalt or a location outside the region
  for a named permanent role, fewer required years than zielprofil_min_jahre, and a mandatory
  formal requirement.
- A preference is never a reason for an exclusion.

## Focus areas, target roles and preferences

- Focus areas (schwerpunkte) are the three to five core skills. A must-have requirement that a
  focus area meets counts twice.
- A matching target role (wunschrollen) adds at most one point. It lifts no cap.
- Preferences (tagessatz_wunsch, remote, regionen and branchen in einsatzpraeferenzen) never
  exclude, and together they move the score at most one point up or down.
- tagessatz_wunsch is a preference, not a lower limit. The lower limit is min_tagessatz.
- If the ad does not name a value, the preference does not count.
- Target role and preferences lift no job to 8 or more while fewer than half of the must-have
  requirements are met.
