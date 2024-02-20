import 'package:flutter/material.dart';
import 'package:get_10101/settings/dlc_channel.dart';

class SubchannelStateLabel extends StatelessWidget {
  final DlcChannel? channel;

  const SubchannelStateLabel({Key? key, this.channel}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    Widget label = _buildLabel("Unknown state", Colors.green.shade300);
    if (channel != null && channel!.subchannelState != null) {
      switch (channel!.subchannelState) {
        case SubchannelState.established:
        case SubchannelState.settled:
          label = _buildLabel("Active", Colors.green.shade300);
          break;
        case SubchannelState.settledOffered:
        case SubchannelState.settledReceived:
        case SubchannelState.settledAccepted:
        case SubchannelState.settledConfirmed:
        case SubchannelState.renewOffered:
        case SubchannelState.renewAccepted:
        case SubchannelState.renewConfirmed:
        case SubchannelState.renewFinalized:
          label = _buildLabel("Pending", Colors.green.shade300);
          break;
        case SubchannelState.closing:
        case SubchannelState.collaborativeCloseOffered:
          label = _buildLabel("Closing", Colors.orange.shade300);
          break;
        case null:
        // nothing
      }
    }
    return label;
  }
}

class ChannelStateLabel extends StatelessWidget {
  final DlcChannel? channel;

  const ChannelStateLabel({Key? key, this.channel}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    Widget label = _buildLabel("Unknown state", Colors.green.shade300);
    if (channel != null) {
      switch (channel!.channelState) {
        case ChannelState.offered:
          label = _buildLabel("Offered", Colors.grey.shade300);
        case ChannelState.accepted:
          label = _buildLabel("Accepted", Colors.grey.shade300);
        case ChannelState.signed:
          label = _buildLabel("Signed", Colors.grey.shade300);
        case ChannelState.closing:
          label = _buildLabel("Closing", Colors.grey.shade300);
        case ChannelState.closed:
          label = _buildLabel("Closed", Colors.grey.shade300);
        case ChannelState.counterClosed:
          label = _buildLabel("Counter closed", Colors.grey.shade300);
        case ChannelState.closedPunished:
          label = _buildLabel("Closed punished", Colors.grey.shade300);
        case ChannelState.collaborativelyClosed:
          label = _buildLabel("Collaboratively closed", Colors.grey.shade300);
        case ChannelState.failedAccept:
          label = _buildLabel("Failed", Colors.grey.shade300);
        case ChannelState.failedSign:
          label = _buildLabel("Failed", Colors.grey.shade300);
        case ChannelState.cancelled:
          label = _buildLabel("Cancelled", Colors.grey.shade300);
      }
    }
    return label;
  }
}

Widget _buildLabel(String text, Color color) {
  return Container(
    decoration: BoxDecoration(
      color: color,
      borderRadius: BorderRadius.circular(15),
    ),
    child: Padding(
      padding: const EdgeInsets.all(8.0),
      child: Center(child: Text(text)),
    ),
  );
}
