import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:provider/provider.dart';

class AmountAndFiatText extends StatelessWidget {
  final Amount amount;

  const AmountAndFiatText({super.key, required this.amount});

  @override
  Widget build(BuildContext context) {
    return Selector<TradeValuesChangeNotifier, double>(
      selector: (_, provider) {
        var askPrice = provider.getAskPrice() ?? 0.0;
        var bidPrice = provider.getBidPrice() ?? 0.0;
        var midMarket = (askPrice + bidPrice) / 2;
        return midMarket;
      },
      builder: (BuildContext context, double price, Widget? child) =>
          Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
        AmountText(amount: amount, textStyle: const TextStyle(fontSize: 17)),
        Wrap(children: [
          Text("~", style: TextStyle(color: Colors.grey.shade700, fontSize: 15)),
          FiatText(
              amount: amount.btc / price,
              textStyle: TextStyle(color: Colors.grey.shade700, fontSize: 15))
        ])
      ]),
    );
  }
}
