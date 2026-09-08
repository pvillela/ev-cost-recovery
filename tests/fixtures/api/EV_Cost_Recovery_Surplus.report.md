EV Cost Recovery Surplus
========================

Period       2026-05-24 - 2026-06-23  (31 days)

| Item             |  Amount |
|:-----------------|--------:|
| Cost recovery    |    2.24 |
| EV energy cost   |   -3.07 |
| EV delivery cost | -136.92 |
| Surplus          | -137.75 |

The cost-recovery rates fell short of the chargers' share of the bill for
this period, by the amount above.

Note: the four amounts above add exactly, because the surplus is computed
from the three as they are printed. The reports below round their own
figures for display, so their columns may not.

EV Cost Recovery
================

Period       2026-05-24 - 2026-06-23  (31 days)

| Item                          | Amount |
|:------------------------------|-------:|
| At rates effective 2026-05-01 |   0.20 |
| At rates effective 2026-06-01 |   2.04 |
| Cost recovery                 |   2.24 |

Note: figures are rounded for display. A column can therefore differ by a
cent, or by a thousandth of a kilowatt-hour, from the total stated for it,
which is computed from the unrounded values.

EV rates effective 2026-05-01  (2026-05-24 - 2026-05-31)
--------------------------------------------------------

| TOU      |   kWh | EV rate | Recovery |
|:---------|------:|--------:|---------:|
| On-peak  | 2.000 | 0.10000 |     0.20 |
| Mid-peak | 0.000 | 0.10000 |     0.00 |
| Off-peak | 0.000 | 0.10000 |     0.00 |
| Total    | 2.000 |         |     0.20 |

EV rates effective 2026-06-01  (2026-06-01 - 2026-06-23)
--------------------------------------------------------

| TOU      |    kWh | EV rate | Recovery |
|:---------|-------:|--------:|---------:|
| On-peak  | 14.000 | 0.12000 |     1.68 |
| Mid-peak |  2.000 | 0.12000 |     0.24 |
| Off-peak |  1.000 | 0.12000 |     0.12 |
| Total    | 17.000 |         |     2.04 |


EV Energy Cost
==============

Period       2026-05-24 - 2026-06-23  (31 days)
Loss factor  1.0295

| Item                            | Amount |
|:--------------------------------|-------:|
| Energy charges                  |   2.87 |
| Wholesale Market Service Charge |   0.11 |
| HST                             |   0.39 |
| Ontario Electricity Rebate      |  -0.30 |
| Energy cost                     |   3.07 |

Note: figures are rounded for display. A column can therefore differ by a
cent, or by a thousandth of a kilowatt-hour, from the total stated for it,
which is computed from the unrounded values.

Energy charges by time of use
-----------------------------

| TOU      |    kWh | Adj. kWh | TH blended rate | Cost |
|:---------|-------:|---------:|----------------:|-----:|
| On-peak  | 16.000 |   16.472 |         0.15385 | 2.53 |
| Mid-peak |  2.000 |    2.059 |         0.12500 | 0.26 |
| Off-peak |  1.000 |    1.030 |         0.07556 | 0.08 |
| Total    | 19.000 |   19.561 |                 | 2.87 |

Wholesale Market Service Charge
-------------------------------

| Basis     | Adj. kWh | TH blended rate | Charge |
|:----------|---------:|----------------:|-------:|
| All bands |   19.561 |         0.00583 |   0.11 |


EV Delivery Cost
================

Period       2026-05-24 - 2026-06-23  (31 days)
Days adj.    31/30 = 1.0333

| Item                       | Amount |
|:---------------------------|-------:|
| Delivery charges           | 132.94 |
| HST                        |  17.28 |
| Ontario Electricity Rebate | -13.29 |
| Delivery cost              | 136.92 |

Note: figures are rounded for display. A column can therefore differ by a
cent, or by a thousandth of a kilowatt-hour, from the total stated for it,
which is computed from the unrounded values.

Delivery charges by component
-----------------------------

| Delivery charges component     | Basis  | EV demand | Adj. demand | TH blended rate | Charge |
|:-------------------------------|:-------|----------:|------------:|----------------:|-------:|
| Distribution Charges           | kVA    |     4.677 |       4.833 |         10.0000 |  48.33 |
| Transmission Connection Charge | kW     |    20.291 |      20.967 |          3.0000 |  62.90 |
| Transmission Network Charge    | kW 7-7 |     4.201 |       4.341 |          5.0000 |  21.70 |
| Total                          |        |           |             |                 | 132.94 |

Each EV demand is the "Energy-based" figure for the 15-minute segment that
charge was priced on -- the energy-based estimate in every case, never the
count-based one. Each is the energy the chargers put into that segment over
the segment's length. The figures, and the interval each was drawn from, are
in the peak power detail report for that charge.



Source Data
===========

Session data
------------

- May.csv
- June.csv

A billing period runs from the 24th of one month to the 23rd of the next, so
it takes as many session reports as it takes to reach across those dates.
They are listed above in the order their names say they begin.

Sessions needing a look
-----------------------

These sessions count towards the figures above, and something about them
needed a judgement call. Only what bears on the above figures is listed.

| File     | Row | Session | Anomaly        |
|:---------|----:|:--------|:---------------|
| June.csv |   7 | HOT     | ExcessiveAvgKw |

- ExcessiveAvgKw - average kilowatts above the Evolute breaker rating at the
  top of the normal voltage band, which the hardware should not allow; the
  session still counts towards every estimate.

Meter data
----------

- TH_Electric_Usage.XML

These hours of the export needed a judgement call. Every figure on the
demand side is a maximum over the whole period, so an hour anywhere in it
that carried no reading is an hour that could have held the maximum and
offered nothing.

| Hour                 | Anomaly    |
|:---------------------|:-----------|
| 2026-06-11 19:00 EDT | MissingKva |

- MissingKva - the hour carried a kWh or kW reading but no kVA.
