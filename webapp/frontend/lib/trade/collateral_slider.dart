import 'dart:math';

import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:syncfusion_flutter_sliders/sliders.dart';
import 'package:syncfusion_flutter_core/theme.dart' as slider_theme;

/// Slider that allows the user to select a value between minValue and maxValue.
class CollateralSlider extends StatefulWidget {
  final int value;
  final Function(int)? onValueChanged;
  final int minValue;
  final int maxValue;
  final String labelText;

  const CollateralSlider(
      {required this.onValueChanged,
      required this.value,
      super.key,
      required this.minValue,
      required this.maxValue,
      required this.labelText});

  @override
  State<CollateralSlider> createState() => _ValueSliderState();
}

class _ValueSliderState extends State<CollateralSlider> {
  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return InputDecorator(
      decoration: InputDecoration(
        border: const OutlineInputBorder(),
        labelText: widget.labelText,
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
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(left: 2, right: 2),
                  child: slider_theme.SfSliderTheme(
                    data: slider_theme.SfSliderThemeData(),
                    child: SfSlider(
                      min: widget.minValue,
                      max: widget.maxValue,
                      value: min(max(widget.value, widget.minValue), widget.maxValue),
                      showTicks: true,
                      stepSize: 1,
                      enableTooltip: true,
                      showLabels: true,
                      tooltipTextFormatterCallback: (dynamic actualValue, String formattedText) {
                        return "$formattedText sats";
                      },
                      labelFormatterCallback: (dynamic actualValue, String formattedText) {
                        if (actualValue == widget.minValue) {
                          return "Min";
                        } else if (actualValue == widget.maxValue) {
                          return "Max";
                        } else {
                          return "";
                        }
                      },
                      tooltipShape: const SfPaddleTooltipShape(),
                      onChanged: widget.onValueChanged == null
                          ? null
                          : (dynamic value) {
                              // weirdly this is a double value
                              widget.onValueChanged!((value as double).toInt());
                            },
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
