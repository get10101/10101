import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/payment_flow.dart';
import 'package:get_10101/features/wallet/domain/wallet_history.dart';
import 'package:intl/intl.dart';
import 'package:timeago/timeago.dart' as timeago;

abstract class WalletHistoryItem extends StatelessWidget {
  abstract final WalletHistoryItemData data;
  static final dateFormat = DateFormat("yyyy-MM-dd HH:mm:ss");

  const WalletHistoryItem({super.key});

  List<Widget> getDetails();
  IconData getFlowIcon();
  bool isOnChain();
  String getTitle();

  @override
  Widget build(BuildContext context) {
    const double statusIconSize = 18;
    Icon statusIcon = switch (data.status) {
      WalletHistoryStatus.pending => const Icon(
          Icons.pending,
          size: statusIconSize,
        ),
      WalletHistoryStatus.confirmed =>
        const Icon(Icons.check_circle, color: Colors.green, size: statusIconSize),
      WalletHistoryStatus.expired =>
        const Icon(Icons.timer_off, color: Colors.red, size: statusIconSize)
    };

    const double flowIconSize = 30;
    Icon flowIcon = Icon(getFlowIcon(), size: flowIconSize);

    String title = getTitle();
    String onOrOff = isOnChain() ? "on-chain" : "off-chain";

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
      child: ListTile(
          onTap: () async {
            await showDialog(context: context, builder: (ctx) => showItemDetails(title, ctx));
          },
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

  Widget showItemDetails(String title, BuildContext context) {
    int directionMultiplier = () {
      switch (data.flow) {
        case PaymentFlow.inbound:
          return 1;
        case PaymentFlow.outbound:
          return -1;
      }
    }();

    return AlertDialog(
      title: Text(title),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          HistoryDetail(
              label: "Amount", value: formatSats(Amount(data.amount.sats * directionMultiplier))),
          HistoryDetail(label: "Date and time", value: dateFormat.format(data.timestamp)),
          ...getDetails(),
        ],
      ),
    );
  }
}

class HistoryDetail extends StatelessWidget {
  final String label;
  final String value;

  const HistoryDetail({super.key, required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8.0),
      child: Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
        Padding(
          padding: const EdgeInsets.only(right: 8.0),
          child: Text(label, style: const TextStyle(fontWeight: FontWeight.bold)),
        ),
        Flexible(child: Text(value)),
      ]),
    );
  }
}

IconData iconForFlow(PaymentFlow flow) {
  switch (flow) {
    case PaymentFlow.inbound:
      return Icons.arrow_downward;
    case PaymentFlow.outbound:
      return Icons.arrow_upward;
  }
}

class LightningPaymentHistoryItem extends WalletHistoryItem {
  @override
  final LightningPaymentData data;
  const LightningPaymentHistoryItem({super.key, required this.data});

  @override
  List<Widget> getDetails() {
    return [
      HistoryDetail(label: "Invoice description", value: data.description),
      Visibility(
        visible: data.invoice != null,
        child: HistoryDetail(label: "Invoice", value: data.invoice ?? ''),
      ),
      HistoryDetail(label: "Payment hash", value: data.paymentHash),
      Visibility(
        visible: data.preimage != null,
        child: HistoryDetail(label: "Payment preimage", value: data.preimage ?? ''),
      ),
    ];
  }

  @override
  IconData getFlowIcon() {
    return iconForFlow(data.flow);
  }

  @override
  String getTitle() {
    return "Payment";
  }

  @override
  bool isOnChain() {
    return false;
  }
}

class TradeHistoryItem extends WalletHistoryItem {
  @override
  final TradeData data;
  const TradeHistoryItem({super.key, required this.data});

  @override
  List<Widget> getDetails() {
    return [HistoryDetail(label: "Order", value: data.orderId)];
  }

  @override
  IconData getFlowIcon() {
    return Icons.bar_chart;
  }

  @override
  String getTitle() {
    switch (data.flow) {
      case PaymentFlow.inbound:
        return "Closed position";
      case PaymentFlow.outbound:
        return "Opened position";
    }
  }

  @override
  bool isOnChain() {
    return false;
  }
}

class OrderMatchingFeeHistoryItem extends WalletHistoryItem {
  @override
  final OrderMatchingFeeData data;
  const OrderMatchingFeeHistoryItem({super.key, required this.data});

  @override
  List<Widget> getDetails() {
    return [
      HistoryDetail(label: "Order", value: data.orderId),
      HistoryDetail(label: "Payment hash", value: data.paymentHash)
    ];
  }

  @override
  IconData getFlowIcon() {
    return Icons.toll;
  }

  @override
  String getTitle() {
    return "Matching fee";
  }

  @override
  bool isOnChain() {
    return false;
  }
}

class JitChannelOpenFeeHistoryItem extends WalletHistoryItem {
  @override
  final JitChannelOpenFeeData data;
  const JitChannelOpenFeeHistoryItem({super.key, required this.data});

  @override
  List<Widget> getDetails() {
    return [HistoryDetail(label: "Funding transaction ID", value: data.txid)];
  }

  @override
  IconData getFlowIcon() {
    return Icons.toll;
  }

  @override
  String getTitle() {
    return "Channel opening fee";
  }

  @override
  bool isOnChain() {
    return false;
  }
}

class OnChainPaymentHistoryItem extends WalletHistoryItem {
  @override
  final OnChainPaymentData data;
  const OnChainPaymentHistoryItem({super.key, required this.data});

  @override
  List<Widget> getDetails() {
    final details = [
      HistoryDetail(label: "Transaction ID", value: data.txid),
      HistoryDetail(label: "Confirmations", value: data.confirmations.toString()),
      Visibility(
        visible: data.fee != null,
        child: HistoryDetail(label: "Fee", value: formatSats(data.fee ?? Amount(0))),
      ),
    ];

    return details;
  }

  @override
  IconData getFlowIcon() {
    return iconForFlow(data.flow);
  }

  @override
  String getTitle() {
    return "Payment";
  }

  @override
  bool isOnChain() {
    return true;
  }
}
