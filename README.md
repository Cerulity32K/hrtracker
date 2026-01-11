# HRTracker
A small application for me to track when to take my HRT.

Schedules are stored in `$HOME/.hrtracker`.

- `hrtracker new name date interval`: Adds a new schedule with the given `name`, starting at the given `date`, and with the given `interval`.
- `hrtracker list` (or simply `hrtracker`): Lists all schedules.
- `hrtracker next name`: Queries when the next event is scheduled for the schedule with the given `name`.
- `hrtracker step name`: Adds the interval of schedule `name` to its date.

## Dates
A date is one of `today` (00:00 of the current day), `tomorrow`/`tmrw` (00:00 of the day after today), or `now` (the current date and time). This can be optionally followed by a `+`, in which case a time will be parsed and added to the date.

## Times
Times are in `hh`, `hh:mm`, or `hh:mm:ss` format, and are parsed as such.
