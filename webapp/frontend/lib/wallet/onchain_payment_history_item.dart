import 'package:flutter/material.dart';
import 'package:get_10101/common/payment.dart';
import 'package:get_10101/wallet/wallet_history_detail_dialog.dart';
import 'package:intl/intl.dart';
import 'package:timeago/timeago.dart' as timeago;

class OnChainPaymentHistoryItem extends StatelessWidget {
  final OnChainPayment data;

  const OnChainPaymentHistoryItem({super.key, required this.data});

  @override
  Widget build(BuildContext context) {
    final formattedDate = DateFormat.yMd().add_jm().format(data.timestamp);

    final statusIcon = switch (data.confirmations) {
      >= 3 => const Icon(Icons.check_circle, color: Colors.green, size: 18),
      _ => const Icon(Icons.pending, size: 18)
    };

    String sign = data.flow == PaymentFlow.inbound ? "+" : "-";
    Color color = data.flow == PaymentFlow.inbound ? Colors.green.shade600 : Colors.red.shade600;
    final flowIcon = data.flow == PaymentFlow.inbound ? Icons.arrow_downward : Icons.arrow_upward;

    var amountFormatter = NumberFormat.compact(locale: "en_UK");

    return Column(
      children: [
        Card(
          color: Colors.transparent,
          margin: const EdgeInsets.all(0),
          elevation: 0,
          child: ListTile(
              onTap: () async {
                showDialog(
                    context: context,
                    builder: (context) {
                      return WalletHistoryDetailDialog(data: data);
                    });
              },
              leading: Stack(children: [
                Container(
                  padding: const EdgeInsets.only(bottom: 20.0),
                  child: SizedBox(height: 18, width: 18, child: statusIcon),
                ),
                Container(
                    padding: const EdgeInsets.only(left: 5.0, top: 10.0),
                    child: SizedBox(height: 30, width: 30, child: Icon(flowIcon, size: 30))),
              ]),
              title: RichText(
                overflow: TextOverflow.ellipsis,
                text: TextSpan(
                  style: DefaultTextStyle.of(context).style,
                  children: const <TextSpan>[
                    TextSpan(text: "Payment"),
                  ],
                ),
              ),
              subtitle: RichText(
                  textWidthBasis: TextWidthBasis.longestLine,
                  text: TextSpan(style: DefaultTextStyle.of(context).style, children: <TextSpan>[
                    TextSpan(
                        text: wasMoreThanHalfAnHourAgo(data.timestamp)
                            ? formattedDate
                            : timeago.format(data.timestamp),
                        style: const TextStyle(color: Colors.grey)),
                  ])),
              trailing: Padding(
                padding: const EdgeInsets.only(top: 5.0, bottom: 1.0),
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    RichText(
                      text: TextSpan(
                          style: DefaultTextStyle.of(context).style,
                          children: <InlineSpan>[
                            TextSpan(
                                text: "$sign${amountFormatter.format(data.amount.sats)} sats",
                                style: TextStyle(
                                    color: color,
                                    fontFamily: "Courier",
                                    fontSize: 14,
                                    fontWeight: FontWeight.bold))
                          ]),
                    ),
                    RichText(
                        text: TextSpan(
                            style: DefaultTextStyle.of(context).style,
                            children: const <TextSpan>[
                          TextSpan(text: "on-chain", style: TextStyle(color: Colors.grey)),
                        ])),
                  ],
                ),
              )),
        ),
        const Divider(height: 0, thickness: 0.5, indent: 10, endIndent: 10)
      ],
    );
  }
}

bool wasMoreThanHalfAnHourAgo(DateTime timestamp) {
  DateTime now = DateTime.now();
  DateTime oneHourAgo = now.subtract(const Duration(minutes: 30));

  return timestamp.isBefore(oneHourAgo);
}
