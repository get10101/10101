import 'package:flutter/material.dart';
import 'package:get_10101/common/dlc_channel_service.dart';
import 'package:get_10101/common/domain/dlc_channel.dart';

enum ChannelStatus {
  unknown,
  notOpen,
  closing,
  withPosition,
  open,
  renewing,
  settling,
}

class DlcChannelChangeNotifier extends ChangeNotifier {
  final DlcChannelService dlcChannelService;

  List<DlcChannel> channels = [];

  DlcChannelChangeNotifier(this.dlcChannelService);

  Future<void> refreshDlcChannels() async {
    channels = await dlcChannelService.getDlcChannels();
    super.notifyListeners();
  }

  Future<void> deleteDlcChannel(String dlcChannelId) async {
    await dlcChannelService.deleteDlcChannel(dlcChannelId);
    await refreshDlcChannels();
  }

  List<SignedDlcChannel> getAllSignedDlcChannels() {
    return channels
        .where((channel) => channel.state == ChannelState.signed)
        .map((channel) => channel as SignedDlcChannel)
        .toList();
  }

  List<OfferedDlcChannel> getAllOfferedDlcChannels() {
    return channels
        .where((channel) => channel.state == ChannelState.offered)
        .map((channel) => channel as OfferedDlcChannel)
        .toList();
  }

  List<AcceptedDlcChannel> getAllAcceptedDlcChannels() {
    return channels
        .where((channel) => channel.state == ChannelState.accepted)
        .map((channel) => channel as AcceptedDlcChannel)
        .toList();
  }

  List<CancelledDlcChannel> getAllCancelledDlcChannels() {
    return channels
        .where((channel) => channel.state == ChannelState.cancelled)
        .map((channel) => channel as CancelledDlcChannel)
        .toList();
  }

  List<ClosingDlcChannel> getAllClosingDlcChannels() {
    return channels
        .where((channel) => channel.state == ChannelState.closing)
        .map((channel) => channel as ClosingDlcChannel)
        .toList();
  }

  List<DlcChannel> getAllOtherDlcChannels() {
    return channels
        .where((channel) => [
              ChannelState.closed,
              ChannelState.counterClosed,
              ChannelState.closedPunished,
              ChannelState.collaborativelyClosed,
              ChannelState.failedAccept,
              ChannelState.failedSign,
              ChannelState.closing,
              ChannelState.offered,
              ChannelState.accepted,
              ChannelState.cancelled,
            ].contains(channel.state))
        .toList();
  }

  ChannelStatus getChannelStatus() {
    if (!channels.any((c) => c.state == ChannelState.signed)) {
      return ChannelStatus.notOpen;
    }

    final channel =
        channels.firstWhere((channel) => channel.state == ChannelState.signed) as SignedDlcChannel;
    switch (channel.signedState) {
      case SignedChannelState.established:
        return ChannelStatus.withPosition;
      case SignedChannelState.settledOffered:
        return ChannelStatus.settling;
      case SignedChannelState.settledReceived:
        return ChannelStatus.settling;
      case SignedChannelState.settledAccepted:
        return ChannelStatus.settling;
      case SignedChannelState.settledConfirmed:
        return ChannelStatus.settling;
      case SignedChannelState.settled:
        return ChannelStatus.open;
      case SignedChannelState.renewOffered:
        return ChannelStatus.renewing;
      case SignedChannelState.renewAccepted:
        return ChannelStatus.renewing;
      case SignedChannelState.renewConfirmed:
        return ChannelStatus.renewing;
      case SignedChannelState.renewFinalized:
        return ChannelStatus.renewing;
      case SignedChannelState.closing:
        return ChannelStatus.notOpen;
      case SignedChannelState.collaborativeCloseOffered:
        return ChannelStatus.unknown;
    }
  }

  bool hasDlcChannel() {
    return channels.any((channel) => channel.state == ChannelState.signed);
  }

  bool hasDlcChannelWithoutPosition() {
    return getAllSignedDlcChannels().any((channel) {
      return [
        SignedChannelState.settled,
        SignedChannelState.settledAccepted,
        SignedChannelState.settledConfirmed
      ].contains(channel.signedState);
    });
  }

  bool canForceClose() {
    return hasDlcChannel();
  }

  /// Whether the current DLC channel is closing or not.
  bool isClosing() {
    return channels.any((channel) =>
        channel.state == ChannelState.closing ||
        (channel is SignedDlcChannel && channel.signedState == SignedChannelState.closing));
  }

  bool hasOpenPosition() {
    return getAllSignedDlcChannels().any((channel) {
      return [
        SignedChannelState.established,
        SignedChannelState.renewAccepted,
        SignedChannelState.renewConfirmed,
        SignedChannelState.renewFinalized,
        SignedChannelState.renewOffered,
        SignedChannelState.settledOffered
      ].contains(channel.signedState);
    });
  }
}
