import 'package:flutter/material.dart';
import 'package:get_10101/common/model.dart';
import 'package:intl/intl.dart';

class AmountText extends StatelessWidget {
  final Amount amount;
  final TextStyle textStyle;

  const AmountText({super.key, required this.amount, this.textStyle = const TextStyle()});

  @override
  Widget build(BuildContext context) {
    return Text(formatAmount(AmountDenomination.satoshi, amount), style: textStyle);
  }
}

String formatAmount(AmountDenomination denomination, Amount amount) {
  switch (denomination) {
    case AmountDenomination.bitcoin:
      return formatBtc(amount);
    case AmountDenomination.satoshi:
      return formatSats(amount);
  }
}

String formatBtc(Amount amount) {
  final formatter = NumberFormat("##,##0.00000000", "en");
  return "${formatter.format(amount.btc)} BTC";
}

String formatSats(Amount amount) {
  final formatter = NumberFormat("#,###,###,###,###", "en");
  return "${formatter.format(amount.sats)} sats";
}

String formatUsd(Usd usd, {int decimalPlaces = 0}) {
  String formatString;
  if (decimalPlaces > 0) {
    formatString = '\$ #,###,###,###,##0.${'0' * decimalPlaces}';
  } else {
    formatString = '\$ #,###,###,###,##0';
  }

  final formatter = NumberFormat(formatString, "en");

  return formatter.format(usd.asDouble);
}

String formatPrice(Price price) {
  final formatter = NumberFormat("\$ #,###,###,###,###", "en");
  return formatter.format(price.usd);
}
