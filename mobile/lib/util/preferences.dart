import 'package:flutter/foundation.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:shared_preferences/shared_preferences.dart';

class Preferences {
  Preferences._privateConstructor();

  static final Preferences instance = Preferences._privateConstructor();

  static const emailAddress = "emailAddress";
  static const openPosition = "openPosition";
  static const fullBackup = "fullBackup";
  static const logLevelTrace = "logLevelTrace";

  Future<bool> setLogLevelTrace(bool trace) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.setBool(logLevelTrace, trace);
  }

  Future<bool> isLogLevelTrace() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getBool(logLevelTrace) ?? kDebugMode;
  }

  Future<bool> setFullBackupRequired(bool required) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.setBool(fullBackup, required);
  }

  Future<bool> isFullBackupRequired() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getBool(fullBackup) ?? true;
  }

  getOpenPosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(openPosition);
  }

  setOpenStablePosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(openPosition, WalletScreen.label);
  }

  setOpenTradePosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.setString(openPosition, TradeScreen.label);
  }

  unsetOpenPosition() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    preferences.remove(openPosition);
  }

  Future<bool> setEmailAddress(String value) async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.setString(emailAddress, value);
  }

  Future<String> getEmailAddress() async {
    SharedPreferences preferences = await SharedPreferences.getInstance();
    return preferences.getString(emailAddress) ?? "";
  }

  Future<bool> hasEmailAddress() async {
    var email = await getEmailAddress();
    return email.isNotEmpty;
  }
}
