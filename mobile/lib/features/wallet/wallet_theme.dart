import 'package:flutter/material.dart';

@immutable
class WalletTheme extends ThemeExtension<WalletTheme> {
  final Color lightning;
  final Color onChain;

  final Color borderColor;
  final Color bgColor;
  final Color dividerColor;
  final ButtonStyle iconButtonStyle;

  WalletTheme({
    this.lightning = Colors.yellow,
    this.onChain = Colors.orange,
    this.borderColor = Colors.grey,
    this.bgColor = Colors.white,
    this.dividerColor = Colors.grey,
    ColorScheme? colors,
    ButtonStyle? iconButtonStyle,
  })  : assert(iconButtonStyle != null || colors != null),
        iconButtonStyle = iconButtonStyle ??
            IconButton.styleFrom(
              foregroundColor: colors!.onPrimary,
              backgroundColor: colors.primary,
              disabledBackgroundColor: colors.onSurface.withOpacity(0.12),
              hoverColor: colors.onPrimary.withOpacity(0.08),
              focusColor: colors.onPrimary.withOpacity(0.12),
              highlightColor: colors.onPrimary.withOpacity(0.12),
            );

  @override
  WalletTheme copyWith({
    Color? lightning,
    Color? onChain,
    Color? borderColor,
    Color? bgColor,
    Color? dividerColor,
    ButtonStyle? iconButtonStyle,
  }) {
    return WalletTheme(
      lightning: lightning ?? this.lightning,
      onChain: onChain ?? this.onChain,
      borderColor: borderColor ?? this.borderColor,
      bgColor: bgColor ?? this.bgColor,
      dividerColor: dividerColor ?? this.dividerColor,
      iconButtonStyle: iconButtonStyle ?? this.iconButtonStyle,
    );
  }

  @override
  WalletTheme lerp(ThemeExtension<WalletTheme>? other, double t) {
    if (other is! WalletTheme) {
      return this;
    }
    return WalletTheme(
      lightning: Color.lerp(lightning, other.lightning, t) ?? Colors.white,
      onChain: Color.lerp(onChain, other.onChain, t) ?? Colors.white,
      borderColor: Color.lerp(borderColor, other.borderColor, t) ?? Colors.white,
      bgColor: Color.lerp(bgColor, other.bgColor, t) ?? Colors.white,
      dividerColor: Color.lerp(dividerColor, other.dividerColor, t) ?? Colors.white,
      iconButtonStyle:
          ButtonStyle.lerp(iconButtonStyle, other.iconButtonStyle, t) ?? const ButtonStyle(),
    );
  }

  @override
  String toString() => 'WalletTheme('
      'lightning: $lightning, '
      'onChain: $onChain, '
      'borderColor: $borderColor, '
      'bgColor: $bgColor, '
      'dividerColor: $dividerColor, '
      'iconButtonStyle: $iconButtonStyle, '
      ')';
}
