import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/middle_ellipsised_text.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/wallet/domain/payment_flow.dart';
import 'package:get_10101/features/wallet/domain/wallet_history.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
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
        const Icon(Icons.timer_off, color: Colors.red, size: statusIconSize),
      WalletHistoryStatus.failed =>
        const Icon(Icons.error, color: Colors.red, size: statusIconSize),
    };

    const double flowIconSize = 30;
    Icon flowIcon = Icon(getFlowIcon(), size: flowIconSize);

    String title = getTitle();
    String onOrOff = isOnChain() ? "on-chain" : "off-chain";

    String sign = switch (data.flow) {
      PaymentFlow.inbound => "+",
      PaymentFlow.outbound => "-",
    };

    Color color = switch (data.flow) {
      PaymentFlow.inbound => Colors.green.shade600,
      PaymentFlow.outbound => Colors.red.shade600,
    };

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
    final (directionMultiplier, verb) = switch ((data.flow, data.status)) {
      (PaymentFlow.inbound, WalletHistoryStatus.failed) => (1, "failed to receive"),
      (PaymentFlow.inbound, WalletHistoryStatus.expired) => (1, "failed to receive"),
      (PaymentFlow.inbound, WalletHistoryStatus.pending) => (1, "are receiving"),
      (PaymentFlow.inbound, WalletHistoryStatus.confirmed) => (1, "received"),
      (PaymentFlow.outbound, WalletHistoryStatus.failed) => (-1, "tried to send"),
      (PaymentFlow.outbound, WalletHistoryStatus.expired) => (-1, "tried to send"),
      (PaymentFlow.outbound, WalletHistoryStatus.confirmed) => (-1, "sent"),
      (PaymentFlow.outbound, WalletHistoryStatus.pending) => (-1, "are sending"),
    };

    int sats = data.amount.sats * directionMultiplier;

    WalletTheme theme = Theme.of(context).extension<WalletTheme>()!;
    HSLColor hsl = HSLColor.fromColor(theme.lightning);
    Color lightningColor = hsl.withLightness(hsl.lightness - 0.15).toColor();

    // TODO(stable): when we have stablesats send & receive, we can
    // set the right icon here
    SvgPicture icon = switch (isOnChain()) {
      true => SvgPicture.asset("assets/Bitcoin_logo.svg"),
      false => SvgPicture.asset("assets/Lightning_logo.svg",
          colorFilter: ColorFilter.mode(lightningColor, BlendMode.srcIn)),
    };

    List<Widget> details = [
      Visibility(
          visible: data.status != WalletHistoryStatus.confirmed,
          child: HistoryDetail(
            label: "Status",
            value: data.status.toString(),
          )),
      HistoryDetail(
          label: "When",
          displayValue: timeago.format(data.timestamp),
          value: dateFormat.format(data.timestamp)),
      ...getDetails(),
    ];

    return AlertDialog(
      title: Text(title, textAlign: TextAlign.center),
      scrollable: true,
      content: Column(mainAxisSize: MainAxisSize.min, children: [
        SizedBox(width: 50, height: 50, child: icon),
        const SizedBox(height: 10),
        Text("You $verb"),
        AmountText(
            amount: Amount(sats),
            textStyle: const TextStyle(fontSize: 25, fontWeight: FontWeight.bold)),
        const SizedBox(height: 10),
        ...details
            .take(details.length - 1)
            .where((child) => child is! Visibility || child.visible)
            .expand((child) => [child, const Divider()]),
        details.last,
      ]),
    );
  }
}

class HistoryDetail extends StatelessWidget {
  final String label;
  final String value;
  final String? displayValue;

  const HistoryDetail({super.key, required this.label, required this.value, this.displayValue});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8.0),
      child: Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
        Padding(
          padding: const EdgeInsets.only(right: 8.0),
          child: Text(label, style: const TextStyle(fontWeight: FontWeight.bold)),
        ),
        Expanded(
          child: Row(mainAxisAlignment: MainAxisAlignment.end, children: [
            Expanded(child: MiddleEllipsisedText(displayValue ?? value)),
            IconButton(
                padding: EdgeInsets.zero,
                onPressed: () {
                  Clipboard.setData(ClipboardData(text: value)).then((_) {
                    showSnackBar(ScaffoldMessenger.of(context), '$label copied to clipboard');
                  });
                },
                icon: const Icon(Icons.copy, size: 18))
          ]),
        )
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
      Visibility(
        visible: data.feeMsats != null && data.flow == PaymentFlow.outbound,
        child: HistoryDetail(label: "Fee", value: "${(data.feeMsats ?? 0) / 1000} sats"),
      ),
      Visibility(
        visible: data.expiry != null,
        child: HistoryDetail(
            label: "Expiry time",
            value: WalletHistoryItem.dateFormat.format(data.expiry ?? DateTime.utc(0))),
      ),
      Visibility(
        visible: data.invoice != null,
        child: HistoryDetail(label: "Lightning invoice", value: data.invoice ?? ''),
      ),
      HistoryDetail(label: "Invoice description", value: data.description),
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
