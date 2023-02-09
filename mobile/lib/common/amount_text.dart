import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_denomination_change_notifier.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';

class AmountText extends StatelessWidget {
  final Amount amount;
  final TextStyle textStyle;

  const AmountText({super.key, required this.amount, this.textStyle = const TextStyle()});

  @override
  Widget build(BuildContext context) {
    AmountDenomination denomination =
        Provider.of<AmountDenominationChangeNotifier>(context).denomination;

    return Text(formatAmount(denomination, amount), style: textStyle);
  }
}

String formatAmount(AmountDenomination denomination, Amount amount) {
  final formatterSat = NumberFormat("#,###,###,###,###", "en");
  final formatterBtc = NumberFormat("##,##0.00000000", "en");

  switch (denomination) {
    case AmountDenomination.bitcoin:
      return "${formatterBtc.format(amount.btc)} BTC";
    case AmountDenomination.satoshi:
      return "${formatterSat.format(amount.sats)} sats";
  }
}
