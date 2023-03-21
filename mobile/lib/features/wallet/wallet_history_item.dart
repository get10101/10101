import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/domain/payment_flow.dart';
import 'package:get_10101/features/wallet/domain/wallet_history.dart';
import 'package:intl/intl.dart';
import 'package:timeago/timeago.dart' as timeago;

class WalletHistoryItem extends StatelessWidget {
  final WalletHistoryItemData data;

  const WalletHistoryItem({super.key, required this.data});

  @override
  Widget build(BuildContext context) {
    const double statusIconSize = 18;
    Icon statusIcon = () {
      switch (data.status) {
        case WalletHistoryStatus.pending:
          return const Icon(
            Icons.pending,
            size: statusIconSize,
          );
        case WalletHistoryStatus.confirmed:
          return const Icon(Icons.check_circle, color: Colors.green, size: statusIconSize);
      }
    }();

    const double flowIconSize = 30;
    Icon flowIcon = () {
      if (data.type == WalletHistoryItemDataType.trade) {
        return const Icon(
          Icons.bar_chart,
          size: flowIconSize,
        );
      }

      switch (data.flow) {
        case PaymentFlow.inbound:
          return const Icon(
            Icons.arrow_downward,
            size: flowIconSize,
          );
        case PaymentFlow.outbound:
          return const Icon(Icons.arrow_upward, size: flowIconSize);
      }
    }();

    String title = () {
      switch (data.type) {
        case WalletHistoryItemDataType.lightning:
          return data.paymentHash ?? "";
        case WalletHistoryItemDataType.onChain:
          return data.txid ?? "";
        case WalletHistoryItemDataType.trade:
          return data.orderId ?? "";
      }
    }();

    String onOrOff = () {
      switch (data.type) {
        case WalletHistoryItemDataType.lightning:
        case WalletHistoryItemDataType.trade:
          return "off-chain";
        case WalletHistoryItemDataType.onChain:
          return "on-chain";
      }
    }();

    String sign = () {
      switch (data.flow) {
        case PaymentFlow.inbound:
          return "+";
        case PaymentFlow.outbound:
          return "-";
      }
    }();

    Color color = () {
      switch (data.flow) {
        case PaymentFlow.inbound:
          return Colors.green.shade600;
        case PaymentFlow.outbound:
          return Colors.red.shade600;
      }
    }();

    var amountFormatter = NumberFormat.compact(locale: "en_IN");

    return Card(
      child: ListTile(
          leading: Stack(children: [
            Container(
              padding: const EdgeInsets.only(bottom: 20.0),
              child: SizedBox(height: statusIconSize, width: statusIconSize, child: statusIcon),
            ),
            Container(
                padding: const EdgeInsets.only(left: 5.0, top: 10.0),
                child: SizedBox(height: flowIconSize, width: flowIconSize, child: flowIcon)),
          ]),
          title: RichText(
            overflow: TextOverflow.ellipsis,
            text: TextSpan(
              style: DefaultTextStyle.of(context).style,
              children: <TextSpan>[
                TextSpan(text: title),
              ],
            ),
          ),
          subtitle: RichText(
              textWidthBasis: TextWidthBasis.longestLine,
              text: TextSpan(style: DefaultTextStyle.of(context).style, children: <TextSpan>[
                TextSpan(
                    text: timeago.format(data.timestamp),
                    style: const TextStyle(color: Colors.grey)),
              ])),
          trailing: Padding(
            padding: const EdgeInsets.only(top: 11.0, bottom: 5.0),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                RichText(
                  text: TextSpan(style: DefaultTextStyle.of(context).style, children: <InlineSpan>[
                    TextSpan(
                        text: "$sign${amountFormatter.format(data.amount.sats)} sats",
                        style: TextStyle(
                            color: color,
                            fontFamily: "Courier",
                            fontSize: 16,
                            fontWeight: FontWeight.bold))
                  ]),
                ),
                RichText(
                    text: TextSpan(style: DefaultTextStyle.of(context).style, children: <TextSpan>[
                  TextSpan(text: onOrOff, style: const TextStyle(color: Colors.grey)),
                ]))
              ],
            ),
          )),
    );
  }
}
