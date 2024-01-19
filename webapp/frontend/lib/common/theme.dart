import 'package:flutter/material.dart';
import 'color.dart';

@immutable
class TenTenOneTheme extends ThemeExtension<TenTenOneTheme> {
  // Unfortunately shades of Colors in flutter are not constant; see https://github.com/flutter/flutter/issues/31351
  // Workaround to make is constant: Initialize the color from HEX
  // For color codes see: https://api.flutter.dev/flutter/material/Colors/green-constant.html
  static const Color green600 = Color(0xFF43A047);
  static const Color red600 = Color(0xFFE53935);

  final Color disabled = Colors.grey;
  static const Color grey300 = Color(0xFF939191);

  final Color buy;
  final Color sell;

  final Color profit;
  final Color loss;

  final Color tabColor;
  final Color leveragePlusButtonColor;
  final Color leverageMinusButtonColor;
  final Color leverageInactiveSliderTrackColor;
  final Color leverageInactiveTicksColor;
  final Color inactiveButtonColor;

  const TenTenOneTheme(
      {this.buy = green600,
      this.sell = red600,
      this.profit = Colors.green,
      this.loss = Colors.red,
      this.inactiveButtonColor = Colors.grey,
      this.tabColor = tenTenOnePurple,
      this.leveragePlusButtonColor = tenTenOnePurple,
      this.leverageMinusButtonColor = tenTenOnePurple,
      this.leverageInactiveSliderTrackColor = Colors.grey,
      this.leverageInactiveTicksColor = Colors.grey});

  @override
  TenTenOneTheme copyWith({
    Color? buy,
    Color? sell,
    Color? profit,
    Color? loss,
    ShapeBorder? tradeButtonShape,
    double? tradeButtonWidth,
    Color? tabColor,
  }) {
    return TenTenOneTheme(
      buy: buy ?? this.buy,
      sell: sell ?? this.sell,
      profit: profit ?? this.profit,
      loss: loss ?? this.loss,
      tabColor: tabColor ?? this.tabColor,
    );
  }

  @override
  TenTenOneTheme lerp(ThemeExtension<TenTenOneTheme>? other, double t) {
    if (other is! TenTenOneTheme) {
      return this;
    }
    return TenTenOneTheme(
      buy: Color.lerp(buy, other.buy, t) ?? Colors.white,
      sell: Color.lerp(sell, other.sell, t) ?? Colors.white,
      profit: Color.lerp(profit, other.profit, t) ?? Colors.white,
      loss: Color.lerp(loss, other.loss, t) ?? Colors.white,
      tabColor: Color.lerp(tabColor, other.tabColor, t) ?? Colors.white,
    );
  }
}
