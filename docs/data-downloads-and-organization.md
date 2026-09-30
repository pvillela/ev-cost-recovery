# Data Downloads and Organization

How to download and organize the data files used by the software.

## Downloading the files

**Hydro bills**

- Go to https://www.torontohydro.com/for-business and login.
- Click the "Accounts & Billing" menu.
- Select "View My Bills".
- Scroll to find the bills you want to download.
- Download the desired bills.

**Green Button meter feed**

- Go to https://www.torontohydro.com/my-account/green-button-data. Login if you are not already logged in for the bills download.
- Select "Usage information" as the type of data to download.
- This building's billing periods run from the 24th of a month to the 23rd of the next month. So, to download usage data for one or more billing periods, do the following:
  - Select the month for the start date. Select the 24th of the month. Select the year.
  - Select the month for the end date. Select the 23rd of the month. Select the year.
  - Click download.

**Evolute files**

Go to https://evolute.ca/#/ and login.

<u>Session report</u> -- It is recommended that you download data for a single month at a time, rather than a report that covers multiple months.

- Click on the "Reporting" menu.
- Select "Session Reports".
- Click on the building listed in the "Select buildings" box.
- Click on the panels shown in the "Select panels" box.
- Do NOT touch the timezone selection. Leave it at "EST (UTC-5)".
- Click on "Select a start date" and select the first day of the desired month.
- Click on "Select an end date" and select the last day of the desired month.
- The appropriate "Report Type" is "Standard Report".
- Click the "Generate Session Reports" button.

<u>Charges report</u> -- You MUST download data for a single month at a time. The software does not accept charges reports spanning more than one month.

- Click on the "Reporting" menu.
- Select "Billing Reports".
- Click on the building listed in the "Select buildings" box.
- Click on "Select a start month" and select the desired month.
- The "Select an end month" should already be set with the same month. If not, change it to be the same as the start month.
- Click "Generate CSV Reports".

## Recommended directory (folder) structure

Organize the application's input and output files as follows:

- Create a base directory, e.g., `EV_Cost_Recovery`.
- Underneath that directory, create the following structure:
  - `evolute` -- contains downloads from Evolute.
  - `green_button` -- contains downloads from Green Button.
  - `hydro_bills` -- contains hydro bills.
  - `rates` -- contains the rates spreadsheet(s), usually just one file which is updated with additional rows as EV cost-recovery rates change over time. See [docs/rates/README.md](rates/README.md).
  - `reports` -- contains the saved reports from the application.
  - `_archive` -- move files to the appropriate sub-folders of `_archive` to unclutter the main data folders as files accumulate over time.