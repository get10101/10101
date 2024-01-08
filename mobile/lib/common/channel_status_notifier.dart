import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/ffi.dart';
import 'package:get_10101/logger/logger.dart';

/// Sends channel status notifications to subscribers.
///
/// Subscribers can learn about the latest [bridge.ChannelStatus] of the LN-DLC channel.
class ChannelStatusNotifier extends ChangeNotifier implements Subscriber {
  bridge.ChannelStatus latest = ChannelStatus.Unknown;

  ChannelStatusNotifier();

  /// Get the latest status of the LN-DLC channel.
  bridge.ChannelStatus getChannelStatus() {
    return latest;
  }

  bool hasDlcChannel() {
    return getChannelStatus() == ChannelStatus.Open;
  }

  bool canForceClose() {
    return getChannelStatus() != ChannelStatus.Unknown ||
        getChannelStatus() != ChannelStatus.NotOpen;
  }

  /// Whether the current LN-DLC channel is closed or not.
  bool isClosing() {
    final status = getChannelStatus();

    return status == ChannelStatus.Closing;
  }

  void subscribe(EventService eventService) {
    eventService.subscribe(this, const bridge.Event.channelStatusUpdate(ChannelStatus.Unknown));
  }

  @override

  /// Handle events coming from the Rust backend.
  ///
  /// We only care about [bridge.Event_ChannelStatusUpdate], as they pertain to
  /// the channel status. If we get a relevant event we update our state and
  /// notify all listeners.
  void notify(bridge.Event event) {
    if (event is bridge.Event_ChannelStatusUpdate) {
      logger.d("Received channel status update: ${event.toString()}");
      latest = event.field0;

      notifyListeners();
    }
  }

  bool hasOpenPosition() {
    return getChannelStatus() == ChannelStatus.WithPosition;
  }
}
