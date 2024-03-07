import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/channel_state_label.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/truncate_text.dart';
import 'package:get_10101/settings/channel_change_notifier.dart';
import 'package:get_10101/settings/channel_service.dart';
import 'package:get_10101/settings/dlc_channel.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

class ChannelScreen extends StatefulWidget {
  const ChannelScreen({super.key});

  @override
  State<ChannelScreen> createState() => _ChannelScreenState();
}

class _ChannelScreenState extends State<ChannelScreen> {
  bool checked = false;
  bool visibility = false;

  List<String> phrase = [];

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    final ChannelChangeNotifier changeNotifier = context.watch<ChannelChangeNotifier>();
    List<DlcChannel> channels = changeNotifier.getChannels() ?? [];
    if (channels.isEmpty) {
      return const Center(child: Text("No channels found"));
    }
    return ListView.separated(
      itemCount: channels.length,
      itemBuilder: (context, index) {
        final channel = channels[index];
        return ListTile(title: ChannelDetailWidget(channel: channel));
      },
      separatorBuilder: (BuildContext context, int index) {
        return const Divider();
      },
    );
  }
}

class ChannelDetailWidget extends StatefulWidget {
  final DlcChannel channel;

  const ChannelDetailWidget({Key? key, required this.channel}) : super(key: key);

  @override
  State<ChannelDetailWidget> createState() => _ChannelDetailWidgetState();
}

class _ChannelDetailWidgetState extends State<ChannelDetailWidget> {
  bool isForceClosed = false;

  @override
  Widget build(BuildContext context) {
    final ChannelService service = context.read<ChannelService>();
    return ExpansionTile(
      initiallyExpanded: widget.channel.channelState == ChannelState.signed &&
          (widget.channel.subchannelState == SubchannelState.settled ||
              widget.channel.subchannelState == SubchannelState.established),
      title: Padding(
        padding: const EdgeInsets.all(4.0),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            const Text("Channel Id"),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                Text(truncateWithEllipsis(20, widget.channel.dlcChannelId ?? "")),
              ],
            ),
          ],
        ),
      ),
      children: [
        SelectionArea(
          child: Padding(
            padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
            child: Column(
              children: [
                buildCopyableField(context, "Channel Id", widget.channel.dlcChannelId),
                Padding(
                  padding: const EdgeInsets.all(4.0),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      const Text("Channel state"),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          ChannelStateLabel(
                            channel: widget.channel,
                          ),
                          IconButton(
                              padding: EdgeInsets.zero,
                              onPressed: () async {
                                showSnackBar(ScaffoldMessenger.of(context),
                                    "Copied ${widget.channel.channelState.name}");
                                await Clipboard.setData(
                                    ClipboardData(text: widget.channel.channelState.name));
                              },
                              icon: const Icon(Icons.copy, size: 18))
                        ],
                      ),
                    ],
                  ),
                ),
                Visibility(
                  visible: widget.channel.subchannelState?.name != null,
                  child: Padding(
                    padding: const EdgeInsets.all(4.0),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        const Text("Subchannel state"),
                        Row(
                          mainAxisAlignment: MainAxisAlignment.end,
                          children: [
                            SubchannelStateLabel(
                              channel: widget.channel,
                            ),
                            IconButton(
                                padding: EdgeInsets.zero,
                                onPressed: () async {
                                  showSnackBar(ScaffoldMessenger.of(context),
                                      "Copied ${widget.channel.subchannelState?.name}");
                                  await Clipboard.setData(ClipboardData(
                                      text: widget.channel.subchannelState?.name ?? ""));
                                },
                                icon: const Icon(Icons.copy, size: 18))
                          ],
                        ),
                      ],
                    ),
                  ),
                ),
                buildCopyableTxId(context, "Funding TxId", widget.channel.fundTxid),
                buildCopyableTxId(context, "Buffer TxId", widget.channel.bufferTxid),
                buildCopyableTxId(context, "Close TxId", widget.channel.closeTxid),
                Visibility(
                    visible: widget.channel.channelState == ChannelState.signed &&
                        widget.channel.subchannelState != null &&
                        (widget.channel.subchannelState! == SubchannelState.established ||
                            widget.channel.subchannelState! == SubchannelState.settled),
                    child: Padding(
                      padding: const EdgeInsets.all(4.0),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          const Text("Force close?"),
                          Checkbox(
                            value: isForceClosed,
                            onChanged: (bool? value) {
                              setState(() {
                                isForceClosed = value!;
                              });
                            },
                          ),
                          ElevatedButton(
                            onPressed: () {
                              showDialog<String>(
                                  context: context,
                                  builder: (BuildContext context) {
                                    String confirmationText =
                                        'Are you sure you want to close this channel?';
                                    if (isForceClosed) {
                                      confirmationText =
                                          'Are you sure you want to force close this channel?';
                                    }
                                    return AlertDialog(
                                      title: const Text('Close channel?'),
                                      content: Text(confirmationText),
                                      actions: <Widget>[
                                        TextButton(
                                          onPressed: () => Navigator.pop(context, 'Cancel'),
                                          child: const Text('Cancel'),
                                        ),
                                        TextButton(
                                          onPressed: () async {
                                            service
                                                .closeChannel(isForceClosed)
                                                .then((value) => showSnackBar(
                                                    ScaffoldMessenger.of(context),
                                                    "Channel will be closed"))
                                                .whenComplete(
                                                    () => Navigator.pop(context, 'Confirm'))
                                                .catchError((error) => showSnackBar(
                                                    ScaffoldMessenger.of(context),
                                                    "Failed at closing channel $error"));
                                          },
                                          child: const Text('Confirm'),
                                        ),
                                      ],
                                    );
                                  });
                            },
                            child: const Text("Close"),
                          ),
                        ],
                      ),
                    ))
              ],
            ),
          ),
        )
      ],
    );
  }

  Visibility buildCopyableTxId(BuildContext context, String title, String? txid) {
    return Visibility(
      visible: txid != null,
      child: Padding(
        padding: const EdgeInsets.all(4.0),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(title),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                Text(
                  truncateWithEllipsis(20, txid ?? ""),
                  maxLines: 1,
                ),
                IconButton(
                    padding: EdgeInsets.zero,
                    onPressed: () async {
                      showSnackBar(ScaffoldMessenger.of(context), "Copied TxId");
                      await Clipboard.setData(ClipboardData(text: txid!));
                    },
                    icon: const Icon(Icons.copy, size: 18)),
                IconButton(
                    padding: EdgeInsets.zero,
                    onPressed: () => launchUrl(
                          buildUri(txid!),
                          webOnlyWindowName: '_blank',
                        ),
                    icon: const Icon(Icons.open_in_new, size: 18))
              ],
            ),
          ],
        ),
      ),
    );
  }

  Visibility buildCopyableField(
    BuildContext context,
    String title,
    String? value,
  ) {
    return Visibility(
      visible: value != null,
      child: Padding(
        padding: const EdgeInsets.all(4.0),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(title),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                Text(truncateWithEllipsis(20, value ?? "")),
                IconButton(
                    padding: EdgeInsets.zero,
                    onPressed: () async {
                      showSnackBar(ScaffoldMessenger.of(context), "Copied $value");
                      await Clipboard.setData(ClipboardData(text: value ?? ""));
                    },
                    icon: const Icon(Icons.copy, size: 18))
              ],
            ),
          ],
        ),
      ),
    );
  }
}

Uri buildUri(String txId) {
  // TODO: support different networks
  return Uri(
    scheme: 'https',
    host: 'mempool.space',
    pathSegments: ['tx', txId],
  );
}
