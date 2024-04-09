import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/domain/fee_estimate.dart';
import 'package:provider/provider.dart';

class FeeText extends StatelessWidget {
  final FeeEstimation fee;

  const FeeText({super.key, required this.fee});

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
        Text("${formatSats(fee.perVbyte)}/vbyte", style: const TextStyle(fontSize: 17)),
        AmountText(amount: fee.total, textStyle: const TextStyle(fontSize: 15)),
        Wrap(children: [
          Text("~", style: TextStyle(color: Colors.grey.shade700, fontSize: 15)),
          FiatText(
              amount: fee.total.btc / price,
              textStyle: TextStyle(color: Colors.grey.shade700, fontSize: 15))
        ])
      ]),
    );
  }
}
