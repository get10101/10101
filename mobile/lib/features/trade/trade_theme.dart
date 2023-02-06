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
  });

  final Color buy;
  final Color sell;

  @override
  TradeTheme copyWith({
    Color? buy,
    Color? sell,
    ShapeBorder? tradeButtonShape,
    double? tradeButtonWidth,
  }) {
    return TradeTheme(
      buy: buy ?? this.buy,
      sell: sell ?? this.sell,
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
    );
  }

  @override
  String toString() => 'TradeTheme('
      'buy: $buy, '
      'sell: $sell, '
      ')';
}
