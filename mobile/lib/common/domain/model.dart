import 'package:decimal/decimal.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:intl/intl.dart';

enum AmountDenomination { bitcoin, satoshi }

class Amount {
  Decimal _sats = Decimal.zero;

  Amount(int sats) {
    _sats = Decimal.fromInt(sats);
  }

  // TODO: this is bad for precision
  Amount.fromBtc(double btc) {
    _sats = Decimal.fromInt((btc * (100000000.0)).round());
  }

  int get sats => _sats.toBigInt().toInt();

  int get toInt => _sats.toBigInt().toInt();

  double get btc => _sats.shift(-8).toDouble();

  double asDouble() => _sats.toDouble();

  Amount.parse(dynamic value) : _sats = Decimal.parse(value);

  Amount.zero() : _sats = Decimal.zero;

  Amount add(Amount amount) {
    return Amount(sats + amount.sats);
  }

  Amount sub(Amount amount) {
    return Amount(sats - amount.sats);
  }

  // Overloading the - operator
  Amount operator -(Amount other) {
    return Amount(sats - other.sats);
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

class Usd {
  Decimal _usd = Decimal.zero;

  Usd(int usd) {
    _usd = Decimal.fromInt(usd);
  }

  int get usd => _usd.toBigInt().toInt();

  int get toInt => _usd.toBigInt().toInt();

  Usd.zero() : _usd = Decimal.zero;

  double asDouble() => _usd.toDouble();

  Usd.fromDouble(double value) : _usd = Decimal.parse(value.toString());
  Usd.parse(dynamic value) : _usd = Decimal.parse(value);

  Usd.parseString(String? value) {
    if (value == null || value.isEmpty) {
      _usd = Decimal.zero;
      return;
    }

    try {
      final f = NumberFormat("#,###");
      int amount =
          // remove any comma and dot from text formatting the users input.
          int.parse(value.replaceAll(f.symbols.GROUP_SEP, ''));

      _usd = Decimal.fromInt(amount);
    } on Exception {
      _usd = Decimal.zero;
    }
  }

  String formatted() {
    final formatter = NumberFormat("#,###,###,###,###", "en");
    return formatter.format(_usd);
  }

  @override
  String toString() {
    return formatUsd(this);
  }
}
