import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/expansion_tile_with_arrow.dart';
import 'package:get_10101/features/wallet/domain/payment_flow.dart';
import 'package:get_10101/features/wallet/domain/wallet_history.dart';
import 'package:intl/intl.dart';
import 'package:timeago/timeago.dart' as timeago;

class WalletHistoryItem extends StatelessWidget {
  final WalletHistoryItemData data;
  static final dateFormat = DateFormat("yyyy-MM-dd HH:mm:ss");

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
      } else if (data.type == WalletHistoryItemDataType.orderMatchingFee) {
        return const Icon(
          Icons.toll,
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
        case WalletHistoryItemDataType.onChain:
          return "Payment";
        case WalletHistoryItemDataType.trade:
          switch (data.flow) {
            case PaymentFlow.inbound:
              return "Closed position";
            case PaymentFlow.outbound:
              return "Opened position";
          }
        case WalletHistoryItemDataType.orderMatchingFee:
          return "Matching fee";
      }
    }();

    String txOrOrder = () {
      switch (data.type) {
        case WalletHistoryItemDataType.lightning:
          return "Payment hash: ${data.paymentHash ?? ''}";
        case WalletHistoryItemDataType.onChain:
          return "Transaction id: ${data.txid ?? ''}";
        case WalletHistoryItemDataType.trade:
        case WalletHistoryItemDataType.orderMatchingFee:
          final orderId = data.orderId!.substring(0, 8);
          return "Order: $orderId";
      }
    }();

    String onOrOff = () {
      switch (data.type) {
        case WalletHistoryItemDataType.lightning:
        case WalletHistoryItemDataType.trade:
        case WalletHistoryItemDataType.orderMatchingFee:
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

    var amountFormatter = NumberFormat.compact(locale: "en_UK");

    return Card(
      child: ExpansionTileWithArrow(
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
                  text: timeago.format(data.timestamp), style: const TextStyle(color: Colors.grey)),
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
        ),
        expandedCrossAxisAlignment: CrossAxisAlignment.start,
        expandedAlignment: Alignment.centerLeft,
        children: [
          Text(txOrOrder),
          Text("Amount: ${formatSats(data.amount)}"),
          Text("Time: ${dateFormat.format(data.timestamp)}")
        ],
      ),
    );
  }
}
