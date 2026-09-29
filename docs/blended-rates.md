# Note about blended rates

The software computes the applicable Toronto Hydro (TH) TOU or delivery charge rate for an item on a bill by dividing the item's amount by the item's quantity (e.g., kWh, kW, 7-7 kW, or kVA, modified by the appropriate adjustment factor). For a billing period during which the Toronto Hydro rates change, the resulting ratios are weighted averages of the rates before and after the change,  We call the ratios *blended rates*. The weights that apply to the two rates that make up a blended rate are respectively proportional to the building's kWh consumption before and after the rate change.

In billing periods during which one or more TH rates change, the use of TH blended rates results in some inaccuracy in the calculation of energy-related (kWh) costs attributable to EV charging activity. That is because the before/after proportion of kWh consumed by the building is almost certainly different from the before/after proportion of kWh consumed by EV charging activity.

Peak power-based charges attributable to EV charging activity are not distorted by the use of TH blended rates. That is because the before-after rate weighting for a delivery charge is based on the billing period's number of calendar before and after the rate change.

Cost-recovery amounts are not done with blended rates. When EV cost-recovery rates change during the billing period, the before and after rates are separately applied to the before and after kWh consumption attributable to EV charging activity.

## 