## Creates a gnuplot diagram fro the payout curve pieces, make sure to enable the CSV creation in the tests

# Set the terminal to PNG and specify the output file
set terminal pngcairo enhanced font "Arial,12" size 800,600

lower_range_short_file = "lower_range_short.csv"
lower_range_long_file = "lower_range_long.csv"
mid_range_short_file = "mid_range_short.csv"
mid_range_long_file = "mid_range_long.csv"
upper_range_short_file = "upper_range_short.csv"
upper_range_long_file = "upper_range_long.csv"
should_file = "should_data_coordinator_short.csv"

# Set the output file name based on the data file
set output "payout_curve.png"

# Define the labels for the X and Y axes
set title "Payout Curve BTCUSD"
set xlabel "Start (in Dollars)"
set ylabel "Payout (in Bitcoin)"

unset ytics
unset xtics
#
unset colorbox

# Define the separator (use semicolon in this case)
separator = ";"

# Specify that the data has a header and set the separator
set datafile separator separator

# Define a conversion factor from sats to Bitcoin (1 Bitcoin = 100,000,000 sats)
conversion_factor = 1e-8

set xtics auto

# Set the Y-axis tics without labels
set ytics

# Set the range for the x-axis from -10 to 110,000
set xrange [-0.1:100000]
set yrange [-0.1:2.5]

set style line 1 linetype 1 linecolor rgb "blue" lw 5
set style line 2 linetype 1 linecolor rgb "green" lw 5
set style line 5 linetype 2 linecolor rgb "pink" lw 2
set style line 6 linetype 2 linecolor rgb "pink" lw 2

# Skip the header row, convert sats to Bitcoin, and create the plot
plot lower_range_short_file using 1:($2 * conversion_factor) ls 1 with lines title "Coordinator Short (Lower range)", \
    lower_range_long_file using 1:($2 * conversion_factor) ls 2 with lines title "Coordinator Long (Lower range)", \
    mid_range_short_file using 1:($2 * conversion_factor) ls 1 with lines title "Coordinator Short (Mid range)", \
    mid_range_long_file using 1:($2 * conversion_factor) ls 2 with lines title "Coordinator Long (Mid range)", \
    upper_range_short_file using 1:($2 * conversion_factor) ls 1 with lines title "Coordinator Short (Upper range)", \
    upper_range_long_file using 1:($2 * conversion_factor) ls 2 with lines title "Coordinator Long (Upper range)", \
    should_file using 1:($2 * conversion_factor) ls 5 with lines title "Should Short", \
    should_file using 1:($3 * conversion_factor) ls 5 with lines title "Should Long"
