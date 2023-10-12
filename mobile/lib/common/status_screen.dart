import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:provider/provider.dart';

class StatusScreen extends StatefulWidget {
  const StatusScreen({super.key});

  @override
  State<StatusScreen> createState() => _StatusScreenState();
}

class _StatusScreenState extends State<StatusScreen> {
  @override
  Widget build(BuildContext context) {
    ServiceStatusNotifier serviceStatusNotifier = context.watch<ServiceStatusNotifier>();

    final orderbookStatus =
        serviceStatusToString(serviceStatusNotifier.getServiceStatus(Service.Orderbook));
    final coordinatorStatus =
        serviceStatusToString(serviceStatusNotifier.getServiceStatus(Service.Coordinator));
    final overallStatus = serviceStatusToString(serviceStatusNotifier.overall());

    ChannelStatusNotifier channelStatusNotifier = context.watch<ChannelStatusNotifier>();

    final channelStatus = channelStatusToString(channelStatusNotifier.getChannelStatus());

    final widgets = [
      Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            children: [
              const SizedBox(height: 20),
              ValueDataRow(
                value: Text(overallStatus),
                label: "Services",
                valueTextStyle: const TextStyle(fontWeight: FontWeight.bold),
              ),
              const Divider(),
              ValueDataRow(
                value: Text(orderbookStatus),
                label: "Orderbook",
                valueTextStyle: const TextStyle(fontWeight: FontWeight.bold),
              ),
              const SizedBox(height: 10),
              ValueDataRow(
                value: Text(coordinatorStatus),
                label: "LSP",
                valueTextStyle: const TextStyle(fontWeight: FontWeight.bold),
              ),
            ],
          )),
      Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            children: [
              const SizedBox(height: 20),
              ValueDataRow(
                value: Text(channelStatus),
                label: "Channel status",
                valueTextStyle: const TextStyle(fontWeight: FontWeight.bold),
              ),
            ],
          )),
      Visibility(
          visible: channelStatusNotifier.isClosing(),
          child: Padding(
              padding: const EdgeInsets.all(32.0),
              child: RichText(
                  text: const TextSpan(
                      style: TextStyle(color: Colors.black, fontSize: 18),
                      children: [
                    TextSpan(
                        text: "Your channel with 10101 is being closed on-chain!\n\n",
                        style: TextStyle(fontWeight: FontWeight.bold)),
                    TextSpan(
                        text:
                            "Your Lightning funds will return back to your on-chain wallet after some time. You will have to reopen the app at some point in the future so that your node can claim them back.\n\n"),
                    TextSpan(
                        text:
                            "If you had a position open your payout will arrive in your on-chain wallet soon after the expiry time. \n")
                  ]))))
    ];

    return Scaffold(
      appBar: AppBar(title: const Text("Status")),
      body: ScrollableSafeArea(
        child: Center(
            child: Column(
          children: widgets,
        )),
      ),
    );
  }

  @override
  void initState() {
    super.initState();
  }
}

String serviceStatusToString(ServiceStatus enumValue) {
  switch (enumValue) {
    case ServiceStatus.Offline:
      return "Offline";
    case ServiceStatus.Online:
      return "Online";
    case ServiceStatus.Unknown:
      return "Unknown";
    default:
      throw Exception("Unknown enum value: $enumValue");
  }
}

String channelStatusToString(ChannelStatus status) {
  switch (status) {
    case ChannelStatus.NotOpen:
      return "Not open";
    case ChannelStatus.LnOpen:
      return "Lightning open";
    case ChannelStatus.LnDlcOpen:
      return "LN-DLC open";
    case ChannelStatus.LnDlcForceClosing:
      return "Force-closing";
    case ChannelStatus.Inconsistent:
      return "Inconsistent";
    case ChannelStatus.Unknown:
      return "Unknown";
  }
}
