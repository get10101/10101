## Creates a gnuplot diagram fro the payout curve

# Set the terminal to PNG and specify the output file
set terminal svg enhanced font "Arial,12" size 800,600

discretized_file_long = "discretized_long.csv"
discretized_file_short = "discretized_short.csv"
should_file_short = "should_short.csv"
should_file_long = "should_long.csv"
computed_file_long = "computed_payout_long.csv"
computed_file_short = "computed_payout_short.csv"

# Define the labels for the X and Y axes
set xlabel "Start (in Dollars)"
set ylabel "Payout (in Bitcoin)"

unset ytics
unset xtics
#
unset colorbox

set key outside

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
set xrange [-0.05:80000]
set yrange [-0.05:1.5]

set style line 1 linetype 1 linecolor rgb "blue" lw 5
set style line 2 linetype 1 linecolor rgb "green" lw 5
set style line 3 linetype 2 linecolor rgb "pink" lw 2
set style line 4 linetype 2 linecolor rgb "violet" lw 2
set style line 5 linetype 2 linecolor rgb "red" lw 2
set style line 6 linetype 2 linecolor rgb "orange" lw 2


# Set the output file for the first diagram
set output "payout_curve.svg"

set multiplot layout 2,1 ;

set title "Payout Curve BTCUSD - From Offerer's Perspective (Long)"

plot discretized_file_long using 1:($2 * conversion_factor) ls 1 with lines title "Discretized Long (Offerer)", \
     discretized_file_long using 1:($3 * conversion_factor) ls 2 with lines title "Discretized Short (Acceptor)", \
     should_file_long using 1:($2 * conversion_factor) ls 3 with lines title "Should Long (Offerer)",  \
     should_file_long using 1:($3 * conversion_factor) ls 4 with lines title "Should Short (Acceptor)",  \
     computed_file_long using 1:($2 * conversion_factor) ls 5 title "Computed Long (Offerer)", \
     computed_file_long using 1:($3 * conversion_factor) ls 6 title "Computed Short (Acceptor)"


# Set the output file for the second diagram
#set output "payout_curve_offerer_short.png"

set title "Payout Curve BTCUSD - From Offerer's Perspective (Short)"

plot discretized_file_short using 1:($2 * conversion_factor) ls 1 with lines title "Discretized Short (Offerer)", \
     discretized_file_short using 1:($3 * conversion_factor) ls 2 with lines title "Discretized Long (Acceptor)", \
     should_file_short using 1:($2 * conversion_factor) ls 3 with lines title "Should Short (Offerer)",  \
     should_file_short using 1:($3 * conversion_factor) ls 4 with lines title "Should Long (Acceptor)",  \
     computed_file_short using 1:($2 * conversion_factor) ls 5 title "Computed Short (Offerer)", \
     computed_file_short using 1:($3 * conversion_factor) ls 6 title "Computed Long (Acceptor)"
