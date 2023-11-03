## Creates a gnuplot diagram fro the payout curve

# Set the terminal to PNG and specify the output file
set terminal pngcairo enhanced font "Arial,12" size 800,600

offerer_long_file = "offerer_long.csv"
offerer_short_file = "offerer_short.csv"
should_file = "should.csv"
computed_file = "computed_payout.csv"

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

set xtics 10000

# Set the Y-axis tics without labels
set ytics 0.1
set grid ytics
set grid xtics

# Set the range for the x-axis to 100,000 max
set xrange [-0.1:80000]
set yrange [-0.1:1]

set style line 1 linetype 1 linecolor rgb "blue" lw 5
set style line 2 linetype 1 linecolor rgb "green" lw 5
set style line 3 linetype 2 linecolor rgb "pink" lw 2
set style line 4 linetype 2 linecolor rgb "violet" lw 2
set style line 5 linetype 2 linecolor rgb "red" lw 2
set style line 6 linetype 2 linecolor rgb "orange" lw 2

plot should_file using 1:($2 * conversion_factor) ls 3 with lines title "Should Short", \
    offerer_short_file using 1:($2 * conversion_factor) ls 2 with lines title "Discretized Short", \
    should_file using 1:($3 * conversion_factor) ls 4 with lines title "Should Long",  \
    offerer_long_file using 1:($2 * conversion_factor) ls 1 with lines title "Discretized Long", \
    computed_file using 1:($2 * conversion_factor) ls 5 title "Computed short", \
    computed_file using 1:($3 * conversion_factor) ls 6 title "Computed long",
