import 'dart:math';
import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:intl/intl.dart';

class PnlLineChart extends StatefulWidget {
  const PnlLineChart({super.key});

  @override
  State<PnlLineChart> createState() => _PnlLineChartState();
}

class _PnlLineChartState extends State<PnlLineChart> {
  final double maxPrice = 100000.0;
  final double quantity = 40000.0;
  final double priceOpening = 40000.0;
  final double leverage = 2.0;
  final List<int> quantityDropdownChoices = <int>[100, 500, 1000, 5000];
  final List<int> leverageChoices = <int>[1, 2, 3, 4, 5];

  late int quantityDropDownChoice;
  late int leverageDropDownChoice;

  @override
  void initState() {
    super.initState();
    quantityDropDownChoice = quantityDropdownChoices.first;
    leverageDropDownChoice = leverageChoices.first;
  }

  Widget bottomTitleWidgets(double value, TitleMeta meta, double chartWidth) {
    if (value % 1 != 0) {
      return Container();
    }

    if (value == meta.max) {
      return const SizedBox.shrink();
    }

    final style = TextStyle(
      color: Colors.black,
      fontWeight: FontWeight.normal,
      fontSize: min(16, 16 * chartWidth / 300),
    );
    return SideTitleWidget(
      axisSide: meta.axisSide,
      space: 16,
      child: Text("\$${meta.formattedValue}", style: style),
    );
  }

  Widget leftTitleWidgets(double value, TitleMeta meta, double chartWidth) {
    final style = TextStyle(
      color: Colors.black,
      fontWeight: FontWeight.normal,
      fontSize: min(16, 16 * chartWidth / 300),
    );

    final formattedValue = "₿${meta.formattedValue}";

    return SideTitleWidget(
      axisSide: meta.axisSide,
      space: 5,
      child: Text(formattedValue, maxLines: 1, style: style),
    );
  }

  @override
  Widget build(BuildContext context) {
    final formatter = NumberFormat.currency(locale: 'en_US', decimalDigits: 0, symbol: '\$');
    var quantity = quantityDropDownChoice.toDouble();
    var leverage = leverageDropDownChoice.toDouble();

    final margin = calculateMargin(quantity, leverage, priceOpening);
    final counterMargin = calculateMargin(quantity, 2.0, priceOpening);
    final spots = calculateFlSpots(quantity, priceOpening, margin, counterMargin, maxPrice);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.center,
      mainAxisAlignment: MainAxisAlignment.spaceEvenly,
      children: [
        Column(
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceEvenly,
              children: [
                Row(
                  children: [
                    const Text('Quantity'),
                    const SizedBox(
                      width: 5,
                    ),
                    DropdownButton<int>(
                      value: quantityDropDownChoice,
                      elevation: 16,
                      style: const TextStyle(color: Colors.deepPurple),
                      underline: Container(
                        height: 2,
                        color: Colors.deepPurpleAccent,
                      ),
                      onChanged: (int? value) {
                        setState(() {
                          quantityDropDownChoice = value!;
                        });
                      },
                      items: quantityDropdownChoices.map<DropdownMenuItem<int>>((int value) {
                        return DropdownMenuItem<int>(
                          value: value,
                          child: Text("\$$value"),
                        );
                      }).toList(),
                    )
                  ],
                ),
                Row(
                  children: [
                    const Text('Leverage'),
                    const SizedBox(
                      width: 5,
                    ),
                    DropdownButton<int>(
                      value: leverageDropDownChoice,
                      elevation: 16,
                      style: const TextStyle(color: Colors.deepPurple),
                      underline: Container(
                        height: 2,
                        color: Colors.deepPurpleAccent,
                      ),
                      onChanged: (int? value) {
                        setState(() {
                          leverageDropDownChoice = value!;
                        });
                      },
                      items: leverageChoices.map<DropdownMenuItem<int>>((int value) {
                        return DropdownMenuItem<int>(
                          value: value,
                          child: Text(
                            "${value}x",
                            textAlign: TextAlign.end,
                          ),
                        );
                      }).toList(),
                    )
                  ],
                ),
              ],
            ),
          ],
        ),
        const SizedBox(
          height: 10,
        ),
        SizedBox(
          height: 170,
          child: Center(
            child: AspectRatio(
              aspectRatio: 2.1,
              child: Padding(
                padding: const EdgeInsets.only(left: 20, right: 40),
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    return LineChart(
                      LineChartData(
                        minX: 0,
                        maxX: maxPrice,
                        lineTouchData: LineTouchData(
                          touchTooltipData: LineTouchTooltipData(
                            maxContentWidth: 100,
                            tooltipBgColor: Colors.transparent,
                            getTooltipItems: (touchedSpots) {
                              return touchedSpots.map((LineBarSpot touchedSpot) {
                                final textStyle = TextStyle(
                                  color:
                                      touchedSpot.bar.gradient?.colors[0] ?? touchedSpot.bar.color,
                                  fontWeight: FontWeight.bold,
                                  fontSize: 14,
                                );
                                return LineTooltipItem(
                                  'at ${formatter.format(touchedSpot.x)} \n you get ₿${touchedSpot.y.toStringAsFixed(8)}',
                                  textStyle,
                                );
                              }).toList();
                            },
                          ),
                          handleBuiltInTouches: true,
                          getTouchLineStart: (data, index) => 0,
                        ),
                        lineBarsData: [
                          LineChartBarData(
                            color: tenTenOnePurple,
                            spots: spots,
                            isCurved: true,
                            isStrokeCapRound: true,
                            barWidth: 3,
                            belowBarData: BarAreaData(
                              show: false,
                            ),
                            dotData: const FlDotData(show: false),
                          ),
                        ],
                        titlesData: FlTitlesData(
                          leftTitles: AxisTitles(
                            sideTitles: SideTitles(
                                showTitles: true,
                                getTitlesWidget: (value, meta) =>
                                    leftTitleWidgets(value, meta, constraints.maxWidth),
                                reservedSize: 66,
                                interval: margin / 2),
                            drawBelowEverything: true,
                          ),
                          rightTitles: const AxisTitles(
                            sideTitles: SideTitles(showTitles: false),
                          ),
                          bottomTitles: AxisTitles(
                            sideTitles: SideTitles(
                              showTitles: true,
                              getTitlesWidget: (value, meta) =>
                                  bottomTitleWidgets(value, meta, constraints.maxWidth),
                              reservedSize: 36,
                              interval: 20000,
                            ),
                            drawBelowEverything: true,
                          ),
                          topTitles: const AxisTitles(
                            sideTitles: SideTitles(showTitles: false),
                          ),
                        ),
                        gridData: FlGridData(
                          show: true,
                          drawHorizontalLine: true,
                          drawVerticalLine: true,
                          checkToShowHorizontalLine: (value) {
                            return value.toInt() == 0;
                          },
                          getDrawingHorizontalLine: (_) => FlLine(
                            color: tenTenOnePurple.withOpacity(1),
                            dashArray: [8, 2],
                            strokeWidth: 0.8,
                          ),
                          getDrawingVerticalLine: (_) => FlLine(
                            color: tenTenOnePurple.withOpacity(1),
                            dashArray: [8, 2],
                            strokeWidth: 0.8,
                          ),
                          checkToShowVerticalLine: (value) {
                            return value.toInt() == 0;
                          },
                        ),
                        borderData: FlBorderData(show: false),
                      ),
                    );
                  },
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

double calculateMargin(double quantity, double leverage, double priceOpening) {
  return quantity / (priceOpening * leverage);
}

double calculatePnL(double quantity, double priceOpening, double priceClosing) {
  double pnl = quantity * ((1 / priceOpening) - (1 / priceClosing));
  return pnl;
}

double calculateLongLiquidationPrice(double leverage, double price) {
  return price * leverage / (leverage + 1.0);
}

/// Calculates the payout in the range from 0 to 100k
List<FlSpot> calculateFlSpots(
    double quantity, double priceOpening, double margin, double counterMargin, double maxPrice) {
  double stepSize = 1000.0;
  return List.generate((maxPrice / stepSize).round() + 1, (index) {
    var closingPrice = index * stepSize;
    var pnl = calculatePnL(quantity, priceOpening, closingPrice);

    if (pnl + margin < 0) {
      pnl = (margin * -1);
    }

    if (pnl > counterMargin) {
      pnl = counterMargin;
    }

    return FlSpot(closingPrice, pnl + margin);
  });
}
