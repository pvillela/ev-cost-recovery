# Reading of EV_Cost_Recovery_Rates.xlsx

- On the "Cost recovery" and "Evolute reimbursement" tabs, replace the direct entry of cost recovery rates with their reading from an Excel file.
- The Excel file can have any name.
- The app will look for a sheet in the workbook with the name "rates" and, if one is not found, it will look for "sheet1".
- In that sheet, the first line must contain the following column names:
  - effective_date -- contents in lines below it must be an Excel date, not a string.
  - on_peak -- contents in lines below it must be an Excel positive number, not a string.
  - mid_peak -- contents in lines below it must be an Excel positive number, not a string.
  - off_peak -- contents in lines below it must be an Excel positive number, not a string.

- Additional columns and content may be present and will be ignored by the app.

- Additional sheets may be present and will be ignored by the app.

- /home/pvillela/DEV/AntiquaryDev/ev-cost-recovery/data/EV_Cost_Recovery_Rates.xlsx is an example of such a workbook.