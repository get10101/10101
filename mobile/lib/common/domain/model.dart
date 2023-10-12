import 'package:decimal/decimal.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:intl/intl.dart';

enum AmountDenomination { bitcoin, satoshi }

class Amount {
  Decimal _sats = Decimal.zero;

  Amount(int sats) {
    _sats = Decimal.fromInt(sats);
  }

  int get sats => _sats.toBigInt().toInt();

  int get toInt => _sats.toBigInt().toInt();

  double get btc => _sats.shift(-8).toDouble();

  double asDouble() => _sats.toDouble();

  Amount.parse(String value) : _sats = Decimal.parse(value);

  Amount.zero() : _sats = Decimal.zero;

  Amount add(Amount amount) {
    return Amount(sats + amount.sats);
  }

  Amount sub(Amount amount) {
    return Amount(sats - amount.sats);
  }

  Amount.parseAmount(String? value) {
    if (value == null || value.isEmpty) {
      _sats = Decimal.zero;
      return;
    }

    try {
      final f = NumberFormat("#,###");
      int amount =
          // remove any comma and dot from text formatting the users input.
          int.parse(value.replaceAll(f.symbols.GROUP_SEP, ''));

      _sats = Decimal.fromInt(amount);
    } on Exception {
      _sats = Decimal.zero;
    }
  }

  String formatted() {
    final formatter = NumberFormat("#,###,###,###,###", "en");
    return formatter.format(sats);
  }

  @override
  String toString() {
    return formatSats(this);
  }
}
