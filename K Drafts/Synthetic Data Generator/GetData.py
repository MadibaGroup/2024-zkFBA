import databento as db

client = db.Historical("My Private API Key")
# Market-By-Price data for a single day
data = client.timeseries.get_range(
    dataset="XNAS.ITCH",
    schema="mbp-10",
    symbols=["AAPL"],
    start="2026-06-01",
    end="2026-06-02"
)
# Converting to DataFrame and cap 1,000 price ticks
df = data.to_df().head(1000)
df.to_csv("nasdaq_depth_1000.csv")
