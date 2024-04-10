import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:intl/intl.dart';
import 'package:syncfusion_flutter_sliders/sliders.dart';
import 'package:syncfusion_flutter_core/theme.dart' as slider_theme;

const double minLeverage = 1.0;
const double maxLeverage = 5.0;

/// Slider that allows the user to select a leverage between minLeverage and maxLeverage.
/// It uses linear scale and fractional leverage values are rounded to the nearest integer.
class LeverageSlider extends StatefulWidget {
  final double initialValue;
  final Function(double) onLeverageChanged;
  final bool isActive;

  const LeverageSlider(
      {required this.onLeverageChanged, this.initialValue = 2, super.key, required this.isActive});

  @override
  State<LeverageSlider> createState() => _LeverageSliderState();
}

class _LeverageSliderState extends State<LeverageSlider> {
  late double _leverage;

  @override
  void initState() {
    _leverage = widget.initialValue;
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return InputDecorator(
      decoration: InputDecoration(
        border: const OutlineInputBorder(),
        labelText: "Leverage",
        labelStyle: const TextStyle(color: tenTenOnePurple),
        filled: true,
        fillColor: Colors.white,
        errorStyle: TextStyle(
          color: Colors.red[900],
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.only(left: 8, right: 8),
        child: SizedBox(
          height: 35,
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              RoundedIconButton(
                icon: FontAwesomeIcons.minus,
                onTap: widget.isActive
                    ? () {
                        setState(() {
                          updateLeverage(_leverage > 1 ? _leverage - 1.0 : _leverage);
                        });
                      }
                    : null,
              ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(left: 2, right: 2),
                  child: slider_theme.SfSliderTheme(
                    data: slider_theme.SfSliderThemeData(
                      activeLabelStyle: const TextStyle(color: Colors.black, fontSize: 12),
                      inactiveLabelStyle: const TextStyle(color: Colors.black45, fontSize: 12),
                      activeTrackColor: tenTenOnePurple.shade50,
                      inactiveTrackColor: tenTenOnePurple.shade50,
                    ),
                    child: SfSlider(
                      min: 1,
                      max: maxLeverage,
                      value: _leverage,
                      stepSize: 1,
                      interval: 1,
                      showTicks: true,
                      showLabels: true,
                      enableTooltip: true,
                      tooltipShape: const SfPaddleTooltipShape(),
                      numberFormat: NumberFormat("x"),
                      tooltipTextFormatterCallback: (dynamic actualValue, String formattedText) {
                        return "${actualValue}x";
                      },
                      onChanged: widget.isActive
                          ? (dynamic value) {
                              updateLeverage(value);
                            }
                          : null,
                    ),
                  ),
                ),
              ),
              RoundedIconButton(
                icon: FontAwesomeIcons.plus,
                onTap: widget.isActive
                    ? () {
                        updateLeverage(_leverage < maxLeverage ? _leverage + 1.0 : maxLeverage);
                      }
                    : null,
              ),
            ],
          ),
        ),
      ),
    );
  }

  updateLeverage(double leverage) {
    setState(() {
      _leverage = leverage;
    });

    widget.onLeverageChanged(_leverage);
  }
}

class LeverageButton extends StatelessWidget {
  const LeverageButton({required this.label, required this.onPressed, super.key});

  final Function onPressed;
  final String label;

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    return SizedBox(
      width: 30,
      height: 30,
      child: ElevatedButton(
          style: ElevatedButton.styleFrom(
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
              padding: EdgeInsets.zero,
              backgroundColor: tradeTheme.leverageMinusButtonColor),
          onPressed: () => onPressed(),
          child: Text(
            label,
            style: const TextStyle(color: Colors.white),
          )),
    );
  }
}

class RoundedIconButton extends StatelessWidget {
  final IconData icon;
  final VoidCallback? onTap;

  const RoundedIconButton({
    Key? key,
    required this.icon,
    required this.onTap,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Container(
        width: 20,
        height: 20,
        decoration: BoxDecoration(
          shape: BoxShape.rectangle,
          color: onTap == null ? Colors.grey : tenTenOnePurple,
          borderRadius: BorderRadius.circular(3),
        ),
        child: Icon(
          icon,
          color: Colors.white,
          size: 16,
        ),
      ),
    );
  }
}
