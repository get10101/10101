import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/trade_theme.dart';

const gradientColors = <Color>[Colors.green, Colors.deepOrange];

const LinearGradient gradient = LinearGradient(colors: gradientColors);

class LeverageSlider extends StatefulWidget {
  final double initialValue;
  final Function(double) onLeverageChanged;

  const LeverageSlider({required this.onLeverageChanged, this.initialValue = 2, super.key});

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
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    return Row(
      children: [
        LeverageButton(
            label: "-",
            onPressed: () {
              if (_leverage > 1) {
                updateLeverage(--_leverage);
              }
            }),
        Expanded(
          child: Column(
            children: [
              Text(
                "Leverage: x${_leverage.round()}",
                style: TextStyle(color: colorFromLeverage()),
              ),
              SliderTheme(
                data: SliderTheme.of(context).copyWith(
                  thumbColor: colorFromLeverage(),
                  trackShape: const GradientRectSliderTrackShape(),
                  overlayShape: SliderComponentShape.noOverlay,
                  inactiveTrackColor: tradeTheme.leverageInactiveSliderTrackColor.withOpacity(0.2),
                  inactiveTickMarkColor: tradeTheme.leverageInactiveTicksColor,
                ),
                child: Slider(
                  value: _leverage,
                  min: 1,
                  max: 10,
                  divisions: 9,
                  label: "x${_leverage.round().toString()}",
                  onChanged: (double value) {
                    updateLeverage(value);
                  },
                ),
              )
            ],
          ),
        ),
        LeverageButton(
            label: "+",
            onPressed: () {
              if (_leverage < 10) {
                updateLeverage(++_leverage);
              }
            })
      ],
    );
  }

  updateLeverage(double leverage) {
    setState(() {
      _leverage = leverage;
    });

    widget.onLeverageChanged(_leverage);
  }

  Color colorFromLeverage() {
    return lerpGradient(gradientColors, <double>[0, 100], _leverage * 10);
  }

  Color lerpGradient(List<Color> colors, List<double> stops, double t) {
    for (var s = 0; s < stops.length - 1; s++) {
      final leftStop = stops[s], rightStop = stops[s + 1];
      final leftColor = colors[s], rightColor = colors[s + 1];
      if (t <= leftStop) {
        return leftColor;
      } else if (t < rightStop) {
        final sectionT = (t - leftStop) / (rightStop - leftStop);
        return Color.lerp(leftColor, rightColor, sectionT) ?? Colors.white;
      }
    }

    return colors.last;
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
          child: Text(label)),
    );
  }
}

/// Code copied from `RoundedRectSliderTrackShape` and modified to use a Gradiant
class GradientRectSliderTrackShape extends SliderTrackShape with BaseSliderTrackShape {
  const GradientRectSliderTrackShape();

  @override
  void paint(
    PaintingContext context,
    Offset offset, {
    required RenderBox parentBox,
    required SliderThemeData sliderTheme,
    required Animation<double> enableAnimation,
    required TextDirection textDirection,
    required Offset thumbCenter,
    Offset? secondaryOffset,
    bool isDiscrete = false,
    bool isEnabled = false,
    double additionalActiveTrackHeight = 2,
  }) {
    assert(sliderTheme.disabledActiveTrackColor != null);
    assert(sliderTheme.disabledInactiveTrackColor != null);
    assert(sliderTheme.activeTrackColor != null);
    assert(sliderTheme.inactiveTrackColor != null);
    assert(sliderTheme.thumbShape != null);

    if (sliderTheme.trackHeight == null || sliderTheme.trackHeight! <= 0) {
      return;
    }

    final Rect trackRect = getPreferredRect(
      parentBox: parentBox,
      offset: offset,
      sliderTheme: sliderTheme,
      isEnabled: isEnabled,
      isDiscrete: isDiscrete,
    );

    final ColorTween activeTrackColorTween =
        ColorTween(begin: sliderTheme.disabledActiveTrackColor, end: sliderTheme.activeTrackColor);
    final ColorTween inactiveTrackColorTween = ColorTween(
        begin: sliderTheme.disabledInactiveTrackColor, end: sliderTheme.inactiveTrackColor);
    final Paint activePaint = Paint()
      ..shader = gradient.createShader(trackRect)
      ..color = activeTrackColorTween.evaluate(enableAnimation)!;
    final Paint inactivePaint = Paint()..color = inactiveTrackColorTween.evaluate(enableAnimation)!;
    final Paint leftTrackPaint;
    final Paint rightTrackPaint;
    switch (textDirection) {
      case TextDirection.ltr:
        leftTrackPaint = activePaint;
        rightTrackPaint = inactivePaint;
        break;
      case TextDirection.rtl:
        leftTrackPaint = inactivePaint;
        rightTrackPaint = activePaint;
        break;
    }

    final Radius trackRadius = Radius.circular(trackRect.height / 2);
    final Radius activeTrackRadius =
        Radius.circular((trackRect.height + additionalActiveTrackHeight) / 2);

    context.canvas.drawRRect(
      RRect.fromLTRBAndCorners(
        trackRect.left,
        (textDirection == TextDirection.ltr)
            ? trackRect.top - (additionalActiveTrackHeight / 2)
            : trackRect.top,
        thumbCenter.dx,
        (textDirection == TextDirection.ltr)
            ? trackRect.bottom + (additionalActiveTrackHeight / 2)
            : trackRect.bottom,
        topLeft: (textDirection == TextDirection.ltr) ? activeTrackRadius : trackRadius,
        bottomLeft: (textDirection == TextDirection.ltr) ? activeTrackRadius : trackRadius,
      ),
      leftTrackPaint,
    );
    context.canvas.drawRRect(
      RRect.fromLTRBAndCorners(
        thumbCenter.dx,
        (textDirection == TextDirection.rtl)
            ? trackRect.top - (additionalActiveTrackHeight / 2)
            : trackRect.top,
        trackRect.right,
        (textDirection == TextDirection.rtl)
            ? trackRect.bottom + (additionalActiveTrackHeight / 2)
            : trackRect.bottom,
        topRight: (textDirection == TextDirection.rtl) ? activeTrackRadius : trackRadius,
        bottomRight: (textDirection == TextDirection.rtl) ? activeTrackRadius : trackRadius,
      ),
      rightTrackPaint,
    );

    final bool showSecondaryTrack = (secondaryOffset != null) &&
        ((textDirection == TextDirection.ltr)
            ? (secondaryOffset.dx > thumbCenter.dx)
            : (secondaryOffset.dx < thumbCenter.dx));

    if (showSecondaryTrack) {
      final ColorTween secondaryTrackColorTween = ColorTween(
          begin: sliderTheme.disabledSecondaryActiveTrackColor,
          end: sliderTheme.secondaryActiveTrackColor);
      final Paint secondaryTrackPaint = Paint()
        ..color = secondaryTrackColorTween.evaluate(enableAnimation)!;
      if (textDirection == TextDirection.ltr) {
        context.canvas.drawRRect(
          RRect.fromLTRBAndCorners(
            thumbCenter.dx,
            trackRect.top,
            secondaryOffset.dx,
            trackRect.bottom,
            topRight: trackRadius,
            bottomRight: trackRadius,
          ),
          secondaryTrackPaint,
        );
      } else {
        context.canvas.drawRRect(
          RRect.fromLTRBAndCorners(
            secondaryOffset.dx,
            trackRect.top,
            thumbCenter.dx,
            trackRect.bottom,
            topLeft: trackRadius,
            bottomLeft: trackRadius,
          ),
          secondaryTrackPaint,
        );
      }
    }
  }
}
