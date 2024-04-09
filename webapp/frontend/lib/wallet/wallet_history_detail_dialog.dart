import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/payment.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/truncate_text.dart';
import 'package:intl/intl.dart';
import 'package:url_launcher/url_launcher.dart';

class WalletHistoryDetailDialog extends StatelessWidget {
  final OnChainPayment data;

  const WalletHistoryDetailDialog({
    Key? key,
    required this.data,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final formattedDate = DateFormat.yMd().add_jm().format(data.timestamp);
    final (directionMultiplier, verb) = switch ((data.flow, data.confirmations)) {
      (PaymentFlow.inbound, 0) => (1, "are receiving"),
      (PaymentFlow.inbound, _) => (1, "received"),
      (PaymentFlow.outbound, 0) => (-1, "are sending"),
      (PaymentFlow.outbound, _) => (-1, "sent"),
    };

    return AlertDialog(
      content: SizedBox(
        width: 440,
        child: Padding(
          padding: const EdgeInsets.only(left: 8.0, right: 8.0, top: 8),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Column(
                children: [
                  SizedBox(
                      width: 50, height: 50, child: SvgPicture.asset("assets/Bitcoin_logo.svg")),
                  Text("You $verb"),
                  AmountText(
                      amount: Amount(data.amount.sats * directionMultiplier),
                      textStyle: const TextStyle(fontSize: 25, fontWeight: FontWeight.bold)),
                ],
              ),
              HistoryDetail(
                label: "When",
                value: formattedDate,
                truncate: false,
              ),
              HistoryDetail(
                  label: "Transaction Id",
                  value: data.txid,
                  displayWidget: TransactionIdText(data.txid)),
              HistoryDetail(label: "Confirmations", value: data.confirmations.toString()),
              HistoryDetail(
                label: "Fee",
                value: data.fee.toString(),
                truncate: false,
              ),
            ],
          ),
        ),
      ),
      actions: [
        ElevatedButton(
          onPressed: () {
            Navigator.of(context).pop();
          },
          child: const Text('Close'),
        ),
      ],
    );
  }
}

class HistoryDetail extends StatelessWidget {
  final String label;
  final String value;
  final Widget? displayWidget;
  final bool truncate;

  static const TextStyle defaultValueStyle = TextStyle(fontSize: 16);

  const HistoryDetail(
      {super.key,
      required this.label,
      required this.value,
      this.displayWidget,
      this.truncate = true});

  @override
  Widget build(BuildContext context) {
    return Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
      Text(label, style: defaultValueStyle.copyWith(fontWeight: FontWeight.bold)),
      Expanded(
        child: Row(children: [
          Expanded(
              child: Align(
                  alignment: Alignment.centerRight,
                  child: displayWidget ??
                      Text(truncate ? truncateWithEllipsis(10, value) : value,
                          style: defaultValueStyle))),
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
    ]);
  }
}

class TransactionIdText extends StatelessWidget {
  final String txId;

  const TransactionIdText(this.txId, {super.key});

  @override
  Widget build(BuildContext context) {
    Uri uri = Uri(
      scheme: 'https',
      host: 'mempool.space',
      pathSegments: ['tx', txId],
    );

    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Text(truncateWithEllipsis(10, txId)),
        IconButton(
            padding: EdgeInsets.zero,
            onPressed: () => launchUrl(uri, mode: LaunchMode.externalApplication),
            icon: const Icon(Icons.open_in_new, size: 18))
      ],
    );
  }
}
