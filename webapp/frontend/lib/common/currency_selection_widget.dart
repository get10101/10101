import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/currency_change_notifier.dart';
import 'package:provider/provider.dart';

class CurrencySelectionScreen extends StatelessWidget {
  const CurrencySelectionScreen({super.key});

  @override
  Widget build(BuildContext context) {
    CurrencyChangeNotifier changeNotifier = context.watch<CurrencyChangeNotifier>();
    final Currency currency = changeNotifier.currency;

    return SegmentedButton<Currency>(
      style: SegmentedButton.styleFrom(
        backgroundColor: Colors.grey[100],
      ),
      segments: <ButtonSegment<Currency>>[
        ButtonSegment<Currency>(
            value: Currency.sats,
            label: Text(Currency.sats.name),
            icon: const Icon(BitcoinIcons.satoshi_v2)),
        ButtonSegment<Currency>(
            value: Currency.btc,
            label: Text(Currency.btc.name),
            icon: const Icon(BitcoinIcons.bitcoin)),
        ButtonSegment<Currency>(
          value: Currency.usd,
          label: Text(Currency.usd.name),
          icon: const Icon(FontAwesomeIcons.dollarSign),
        ),
      ],
      selected: <Currency>{currency},
      onSelectionChanged: (Set<Currency> newSelection) {
        changeNotifier.currency = newSelection.first;
      },
    );
  }
}
