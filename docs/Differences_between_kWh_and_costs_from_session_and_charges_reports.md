# Differences between kWh and costs computed from session and charges reports

- The same database records feed the two reports, which perform different aggregations of the source data. 
- The charges report is based on the month-do-date aggregation of billing information (energy and costs), whereas the session report's focus is on the temporal distribution of energy use. 
- Calendar month energy totals obtained from the two reports may differ due to trickle charging thresholds for session reporting, rounding, reporting cut-off times, and non-uniform energy consumption over sessions that cross month boundaries.
- A calendar month dollar total obtained by applying the EV cost-recovery rates to TOU kWh values from a session report may differ from the corresponding dollar total for the same month's charges report for all of the above reasons, as well as non-uniform energy consumption over sessions that cross TOU boundaries.