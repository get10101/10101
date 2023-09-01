import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/util/preferences.dart';

/// Updates `positionExpiry` Preferences value based on the current state of the position.
class PositionExpiryObserver {
  final PositionChangeNotifier _positionChangeNotifier;
  DateTime? _cachedExpiry;

  PositionExpiryObserver(this._positionChangeNotifier) {
    _init();
  }

  _init() async {
    _positionChangeNotifier.addListener(_onPositionChange);
  }

  _onPositionChange() {
    _updatePositionExpiryInPreferences();
  }

  _updatePositionExpiryInPreferences() async {
    if (_positionChangeNotifier.positions.length > 1) {
      throw Exception('More than one position at a time is not supported');
    }
    final positionUsd = _positionChangeNotifier.positions[ContractSymbol.btcusd];

    // Only update if expiry has changed
    if (positionUsd?.expiry != _cachedExpiry) {
      _cachedExpiry = positionUsd?.expiry;
      if (positionUsd == null) {
        Preferences.instance.clearPositionExpiry();
      } else {
        Preferences.instance.setPositionExpiry(positionUsd.expiry);
      }
    }
  }
}
