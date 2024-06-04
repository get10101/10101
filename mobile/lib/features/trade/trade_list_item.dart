import 'package:timeago/timeago.dart' as timeago;
import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/trade.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:intl/intl.dart';
import 'package:get_10101/features/trade/contract_symbol_icon.dart';

class TradeListItem extends StatelessWidget {
  const TradeListItem({super.key, required this.trade});

  final Trade trade;

  @override
  Widget build(BuildContext context) {
    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    final formatter = NumberFormat();
    formatter.minimumFractionDigits = 2;
    formatter.maximumFractionDigits = 2;

    String tradeTypeText;
    switch (trade.tradeType) {
      case TradeType.trade:
        tradeTypeText = "Trade";
      case TradeType.funding:
        tradeTypeText = "Funding";
    }

    var pnlTextSpan = trade.pnl != null && trade.pnl!.sats != 0
        ? <TextSpan>[
            const TextSpan(text: "PNL: "),
            TextSpan(
                text: "${trade.pnl}\n",
                style: TextStyle(
                    color:
                        trade.pnl!.sats.isNegative ? Colors.red.shade600 : Colors.green.shade600)),
          ]
        : <TextSpan>[];

    var feeTextSpan = trade.fee.sats != 0
        ? <TextSpan>[
            const TextSpan(text: "Fee: "),
            TextSpan(
                text: "${trade.fee}\n",
                style: TextStyle(
                    color:
                        trade.fee.sats.isNegative ? Colors.red.shade600 : Colors.green.shade600)),
          ]
        : <TextSpan>[];

    return Column(
      children: [
        Card(
          margin: const EdgeInsets.all(0),
          elevation: 0,
          child: ListTile(
            leading: const Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                ContractSymbolIcon(
                  height: 20,
                  width: 20,
                  paddingUsd: EdgeInsets.only(left: 12.0),
                ),
              ],
            ),
            title: RichText(
              text: TextSpan(
                style: DefaultTextStyle.of(context).style,
                children: <TextSpan>[
                  TextSpan(
                      text: trade.direction.nameU,
                      style: TextStyle(
                          color:
                              trade.direction == Direction.long ? tradeTheme.buy : tradeTheme.sell,
                          fontWeight: FontWeight.bold)),
                  TextSpan(
                      text: " ${trade.quantity} ",
                      style: const TextStyle(fontWeight: FontWeight.bold)),
                  const TextSpan(text: "@ ", style: TextStyle(color: Colors.grey)),
                  TextSpan(text: "${trade.price}")
                ],
              ),
            ),
            trailing: RichText(
              text: TextSpan(
                  style: DefaultTextStyle.of(context).style,
                  children: [TextSpan(text: tradeTypeText)]),
            ),
            subtitle: RichText(
                textWidthBasis: TextWidthBasis.longestLine,
                text: TextSpan(style: DefaultTextStyle.of(context).style, children: <TextSpan>[
                  ...pnlTextSpan,
                  ...feeTextSpan,
                  TextSpan(
                      text: timeago.format(trade.timestamp),
                      style: const TextStyle(color: Colors.grey)),
                ])),
          ),
        ),
        const Divider(height: 0, thickness: 1, indent: 10, endIndent: 10)
      ],
    );
  }
}
