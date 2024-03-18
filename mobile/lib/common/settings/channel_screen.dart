import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/domain/dlc_channel.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/wallet_history_item.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class ChannelScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "channel";

  const ChannelScreen({
    super.key,
  });

  @override
  State<ChannelScreen> createState() => _ChannelScreenState();
}

class _ChannelScreenState extends State<ChannelScreen> {
  bool isCloseChannelButtonDisabled = false;

  @override
  void initState() {
    super.initState();

    DlcChannelChangeNotifier dlcChannelChangeNotifier = context.read<DlcChannelChangeNotifier>();
    dlcChannelChangeNotifier.refreshDlcChannels();
  }

  @override
  Widget build(BuildContext context) {
    DlcChannelChangeNotifier dlcChannelChangeNotifier = context.watch<DlcChannelChangeNotifier>();

    final channelStatus = channelStatusToString(dlcChannelChangeNotifier.getChannelStatus());

    final signedChannels = dlcChannelChangeNotifier.getAllSignedDlcChannels();

    final otherChannels = [
      ...dlcChannelChangeNotifier.getAllOfferedDlcChannels(),
      ...dlcChannelChangeNotifier.getAllAcceptedDlcChannels(),
      ...dlcChannelChangeNotifier.getAllCancelledDlcChannels(),
      ...dlcChannelChangeNotifier.getAllClosingDlcChannels(),
      ...dlcChannelChangeNotifier.getAllSettledClosingDlcChannels(),
      ...dlcChannelChangeNotifier.getAllClosedDlcChannels(),
      ...dlcChannelChangeNotifier.getAllOtherDlcChannels()
    ];

    return Scaffold(
      body: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
          child: SafeArea(
            child: Column(
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Expanded(
                      child: Stack(
                        children: [
                          GestureDetector(
                            child: Container(
                                alignment: AlignmentDirectional.topStart,
                                decoration: BoxDecoration(
                                    color: Colors.transparent,
                                    borderRadius: BorderRadius.circular(10)),
                                width: 70,
                                child: const Icon(
                                  Icons.arrow_back_ios_new_rounded,
                                  size: 22,
                                )),
                            onTap: () {
                              GoRouter.of(context).pop();
                            },
                          ),
                          const Row(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              Text(
                                "Channel",
                                style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                const SizedBox(
                  height: 10,
                ),
                Expanded(
                  child: SingleChildScrollView(
                      child: Column(
                    children: [
                      Padding(
                          padding: const EdgeInsets.only(
                              top: 10.0, bottom: 20.0, left: 14.0, right: 14.0),
                          child: Column(
                            children: [
                              const SizedBox(height: 20),
                              ValueDataRow(
                                type: ValueType.text,
                                value: channelStatus,
                                label: "Channel status",
                                labelTextStyle: const TextStyle(fontSize: 18),
                                valueTextStyle:
                                    const TextStyle(fontWeight: FontWeight.bold, fontSize: 18),
                              ),
                            ],
                          )),
                      const Divider(height: 1, thickness: 1, indent: 5, endIndent: 5),
                      Column(
                        children: <Widget>[
                          const SizedBox(height: 10.0),
                          Theme(
                            data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
                            child: ExpansionTile(
                                initiallyExpanded: signedChannels.isNotEmpty,
                                title: Text(
                                  "Signed Channels (${signedChannels.length})",
                                  style:
                                      const TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
                                ),
                                children: signedChannels.map((channel) {
                                  return ExpansionTile(
                                    title: Text(
                                      channel.signedState.toString(),
                                      style: const TextStyle(fontSize: 17),
                                    ),
                                    children: <Widget>[
                                      ListTile(
                                        leading: const Text('DLC Channel Id',
                                            style: TextStyle(fontSize: 17)),
                                        title: IdText(id: channel.id, length: 8),
                                      ),
                                      ListTile(
                                          leading: const Text('Contract Id',
                                              style: TextStyle(fontSize: 17)),
                                          title: IdText(id: channel.contractId ?? "n/a")),
                                      ListTile(
                                        leading: const Text('Funding TXID',
                                            style: TextStyle(fontSize: 17)),
                                        title: TransactionIdText(channel.fundingTxid),
                                      ),
                                      channel.closingTxid != null
                                          ? ListTile(
                                              leading: const Text('Closing TXID',
                                                  style: TextStyle(fontSize: 17)),
                                              title: TransactionIdText(channel.closingTxid!))
                                          : Container(),
                                    ],
                                  );
                                }).toList()),
                          ),
                          ChannelsTile(title: "Other Channels", channels: otherChannels)
                        ],
                      ),
                      Visibility(
                          visible: dlcChannelChangeNotifier.isClosing(),
                          child: Padding(
                              padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
                              child: RichText(
                                  text: const TextSpan(
                                      style: TextStyle(color: Colors.black, fontSize: 18),
                                      children: [
                                    TextSpan(
                                        text:
                                            "Your channel with 10101 is being closed on-chain!\n\n",
                                        style: TextStyle(fontWeight: FontWeight.bold)),
                                    TextSpan(
                                        text:
                                            "Your off-chain funds will return back to your on-chain wallet after some time.\n\n"),
                                    TextSpan(
                                        text:
                                            "If you had a position open your payout will arrive in your on-chain wallet soon after the expiry time. \n")
                                  ]))))
                    ],
                  )),
                ),
              ],
            ),
          )),
    );
  }
}

class ChannelsTile extends StatelessWidget {
  final String title;
  final List<DlcChannel> channels;

  const ChannelsTile({super.key, required this.title, required this.channels});

  @override
  Widget build(BuildContext context) {
    return Theme(
      data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
      child: ExpansionTile(
          title: Text(
            "$title (${channels.length})",
            style: const TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
          ),
          children: channels.map((channel) {
            return ExpansionTile(
              title: Text(channel.state.toString()),
              children: <Widget>[
                ListTile(
                    leading: const Text('DLC Channel Id', style: TextStyle(fontSize: 17)),
                    title: IdText(id: channel.id)),
                Visibility(
                  visible: channel.getContractId() != null,
                  child: ListTile(
                      leading: const Text('Contract Id', style: TextStyle(fontSize: 17)),
                      title: IdText(id: channel.getContractId() ?? "n/a")),
                ),
                channel is ClosingDlcChannel
                    ? ListTile(
                        leading: const Text('Buffer TXID', style: TextStyle(fontSize: 17)),
                        title: TransactionIdText(channel.bufferTxid))
                    : Container(),
                channel is SettledClosingDlcChannel
                    ? ListTile(
                        leading: const Text('Settle TXID', style: TextStyle(fontSize: 17)),
                        title: TransactionIdText(channel.settleTxid))
                    : Container(),
                channel is ClosedDlcChannel
                    ? ListTile(
                        leading: const Text('Closing TXID', style: TextStyle(fontSize: 17)),
                        title: TransactionIdText(channel.closingTxid))
                    : Container(),
              ],
            );
          }).toList()),
    );
  }
}

class IdText extends StatelessWidget {
  final String id;
  final int length;

  const IdText({super.key, required this.id, this.length = 10});

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Text(truncateWithEllipsis(length, id)),
        IconButton(
            padding: EdgeInsets.zero,
            onPressed: () => Clipboard.setData(ClipboardData(text: id)).then((_) {
                  showSnackBar(ScaffoldMessenger.of(context), "Id copied");
                }),
            icon: const Icon(Icons.copy, size: 18))
      ],
    );
  }
}

String channelStatusToString(ChannelStatus status) {
  switch (status) {
    case ChannelStatus.notOpen:
      return "Not open";
    case ChannelStatus.withPosition:
      return "With Position";
    case ChannelStatus.renewing:
    case ChannelStatus.settling:
      return "Pending";
    case ChannelStatus.closing:
      return "Closing";
    case ChannelStatus.unknown:
      return "Unknown";
    case ChannelStatus.open:
      return "Open";
  }
}
