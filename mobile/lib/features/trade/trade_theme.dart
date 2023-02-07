import 'package:flutter/material.dart';

@immutable
class TradeTheme extends ThemeExtension<TradeTheme> {
  // Unfortunately shades of Colors in flutter are not constant; see https://github.com/flutter/flutter/issues/31351
  // Workaround to make is constant: Initialize the color from HEX
  // For color codes see: https://api.flutter.dev/flutter/material/Colors/green-constant.html
  static const Color green600 = Color(0xFF43A047);
  static const Color red600 = Color(0xFFE53935);

  const TradeTheme({
    this.buy = green600,
    this.sell = red600,
    this.tabColor = Colors.grey,
  });

  final Color buy;
  final Color sell;

  final Color tabColor;

  @override
  TradeTheme copyWith({
    Color? buy,
    Color? sell,
    ShapeBorder? tradeButtonShape,
    double? tradeButtonWidth,
    Color? tabColor,
  }) {
    return TradeTheme(
      buy: buy ?? this.buy,
      sell: sell ?? this.sell,
      tabColor: tabColor ?? this.tabColor,
    );
  }

  @override
  TradeTheme lerp(ThemeExtension<TradeTheme>? other, double t) {
    if (other is! TradeTheme) {
      return this;
    }
    return TradeTheme(
      buy: Color.lerp(buy, other.buy, t) ?? Colors.white,
      sell: Color.lerp(sell, other.sell, t) ?? Colors.white,
      tabColor: Color.lerp(tabColor, other.tabColor, t) ?? Colors.white,
    );
  }

  @override
  String toString() => 'TradeTheme('
      'buy: $buy, '
      'sell: $sell, '
      'tabColor: $tabColor, '
      ')';
}
