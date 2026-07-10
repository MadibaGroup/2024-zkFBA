from datetime import datetime
import databento as db
import pandas as pd
from zoneinfo import ZoneInfo

target_symbol = "AAPL"
target_date = "2026-06-01"

# market hours (9:30 AM to 4:00 PM)
custom_start_hour, custom_start_minute = 9, 30
custom_end_hour, custom_end_minute = 16, 0

ny_tz = ZoneInfo("America/New_York")

custom_start = datetime.strptime(
    f"{target_date} {custom_start_hour}:{custom_start_minute}:00",
    "%Y-%m-%d %H:%M:%S",
).replace(tzinfo=ny_tz)

custom_end = datetime.strptime(
    f"{target_date} {custom_end_hour}:{custom_end_minute}:00",
    "%Y-%m-%d %H:%M:%S",
).replace(tzinfo=ny_tz)

# Fetch data
client = db.Historical("YOUR_API_KEY")
data = client.timeseries.get_range(
    dataset="XNAS.ITCH",
    schema="mbp-10",
    symbols=[target_symbol],
    start=custom_start,
    end=custom_end,
)

df = data.to_df()

# Convert index timestamps from UTC to New York (Eastern) Time
if not df.empty:

    df.index = pd.to_datetime(df.index, utc=True)
    df.index = df.index.tz_convert("America/New_York")

df_capped = df.head(1000)

output_file = f"nasdaq_local_time_{target_symbol}.csv"
df_capped.to_csv(output_file)
